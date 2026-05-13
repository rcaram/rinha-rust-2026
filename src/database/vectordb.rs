use crate::database::simd::l2_distance;
use memmap2::Mmap;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::mem::{align_of, size_of};
use std::path::Path;
use std::sync::{Arc, OnceLock};

const VECTOR_DIM: usize = 14;
const HEADER_SIZE: usize = 16;
const BINARY_MAGIC: u32 = 0x5645_4352;
const BINARY_VERSION: u32 = 1;

#[derive(Debug)]
pub enum VectorStoreError {
    Io(std::io::Error),
    InvalidVectorLength { expected: usize, got: usize },
    EmptyDataset,
    InvalidK,
    StoreUnavailable(String),
    InvalidBinary(String),
}

impl Display for VectorStoreError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "io error: {error}"),
            Self::InvalidVectorLength { expected, got } => {
                write!(f, "invalid vector length: expected {expected}, got {got}")
            }
            Self::EmptyDataset => write!(f, "vector store is empty"),
            Self::InvalidK => write!(f, "k must be greater than zero"),
            Self::StoreUnavailable(reason) => write!(f, "store unavailable: {reason}"),
            Self::InvalidBinary(reason) => write!(f, "invalid binary vector source: {reason}"),
        }
    }
}

impl std::error::Error for VectorStoreError {}

impl From<std::io::Error> for VectorStoreError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

fn parse_u32_le(bytes: &[u8], start: usize) -> Result<u32, VectorStoreError> {
    let end = start + size_of::<u32>();
    let raw = bytes
        .get(start..end)
        .ok_or_else(|| VectorStoreError::InvalidBinary("truncated header".to_string()))?;
    let mut buffer = [0_u8; 4];
    buffer.copy_from_slice(raw);
    Ok(u32::from_le_bytes(buffer))
}

fn cast_bytes_to_f32(bytes: &[u8]) -> Result<&[f32], VectorStoreError> {
    if !bytes.len().is_multiple_of(size_of::<f32>()) {
        return Err(VectorStoreError::InvalidBinary(
            "vectors section has invalid byte size".to_string(),
        ));
    }
    if !(bytes.as_ptr() as usize).is_multiple_of(align_of::<f32>()) {
        return Err(VectorStoreError::InvalidBinary(
            "vectors section has invalid alignment".to_string(),
        ));
    }
    let len = bytes.len() / size_of::<f32>();
    // SAFETY: size and alignment checked above, data remains valid while mmap is alive.
    Ok(unsafe { std::slice::from_raw_parts(bytes.as_ptr() as *const f32, len) })
}

#[derive(Debug)]
pub struct VectorStore {
    mmap: Mmap,
    dim: usize,
    rows: usize,
    vectors_offset: usize,
    labels_offset: usize,
}

#[derive(Clone, Copy, Debug)]
struct ScoredNeighbor {
    distance: f32,
    is_fraud: bool,
}

impl PartialEq for ScoredNeighbor {
    fn eq(&self, other: &Self) -> bool {
        self.distance.to_bits() == other.distance.to_bits() && self.is_fraud == other.is_fraud
    }
}

impl Eq for ScoredNeighbor {}

impl PartialOrd for ScoredNeighbor {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ScoredNeighbor {
    fn cmp(&self, other: &Self) -> Ordering {
        self.distance
            .partial_cmp(&other.distance)
            .unwrap_or(Ordering::Equal)
    }
}

impl VectorStore {
    pub fn from_mmap(path: &str) -> Result<Self, VectorStoreError> {
        let file = File::open(path)?;
        // SAFETY: file descriptor remains valid during mapping call.
        let mmap = unsafe { Mmap::map(&file)? };
        let header = mmap
            .get(0..HEADER_SIZE)
            .ok_or_else(|| VectorStoreError::InvalidBinary("header too short".to_string()))?;

        let magic = parse_u32_le(header, 0)?;
        if magic != BINARY_MAGIC {
            return Err(VectorStoreError::InvalidBinary(format!(
                "unexpected magic value {magic}"
            )));
        }

        let version = parse_u32_le(header, 4)?;
        if version != BINARY_VERSION {
            return Err(VectorStoreError::InvalidBinary(format!(
                "unsupported version {version}"
            )));
        }

        let dim = parse_u32_le(header, 8)? as usize;
        if dim != VECTOR_DIM {
            return Err(VectorStoreError::InvalidVectorLength {
                expected: VECTOR_DIM,
                got: dim,
            });
        }

        let rows = parse_u32_le(header, 12)? as usize;
        let vectors_bytes = rows
            .checked_mul(dim)
            .and_then(|value| value.checked_mul(size_of::<f32>()))
            .ok_or_else(|| {
                VectorStoreError::InvalidBinary("vectors payload overflow".to_string())
            })?;
        let labels_offset = HEADER_SIZE
            .checked_add(vectors_bytes)
            .ok_or_else(|| VectorStoreError::InvalidBinary("binary offset overflow".to_string()))?;
        let labels_end = labels_offset.checked_add(rows).ok_or_else(|| {
            VectorStoreError::InvalidBinary("labels payload overflow".to_string())
        })?;

        if mmap.len() != labels_end {
            return Err(VectorStoreError::InvalidBinary(format!(
                "unexpected file size: got {}, expected {labels_end}",
                mmap.len()
            )));
        }

        let vectors_bytes_slice = &mmap[HEADER_SIZE..labels_offset];
        let _ = cast_bytes_to_f32(vectors_bytes_slice)?;

        Ok(Self {
            mmap,
            dim,
            rows,
            vectors_offset: HEADER_SIZE,
            labels_offset,
        })
    }

    fn vectors(&self) -> Result<&[f32], VectorStoreError> {
        cast_bytes_to_f32(&self.mmap[self.vectors_offset..self.labels_offset])
    }

    fn labels(&self) -> &[u8] {
        &self.mmap[self.labels_offset..]
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rows == 0
    }

    pub fn query_fraud_hits(&self, query: &[f32], k: usize) -> Result<i8, VectorStoreError> {
        if k == 0 {
            return Err(VectorStoreError::InvalidK);
        }
        if self.is_empty() {
            return Err(VectorStoreError::EmptyDataset);
        }
        if query.len() != self.dim {
            return Err(VectorStoreError::InvalidVectorLength {
                expected: self.dim,
                got: query.len(),
            });
        }

        let vectors = self.vectors()?;
        let labels = self.labels();
        let total = labels.len();
        let limit = k.min(total);
        let mut top_k = BinaryHeap::with_capacity(limit);

        for (idx, label) in labels.iter().enumerate() {
            let start = idx * self.dim;
            let end = start + self.dim;
            let distance = l2_distance(query, &vectors[start..end]);
            let neighbor = ScoredNeighbor {
                distance,
                is_fraud: *label != 0,
            };

            if top_k.len() < limit {
                top_k.push(neighbor);
                continue;
            }

            if let Some(worst) = top_k.peek() {
                if neighbor.distance < worst.distance {
                    let _ = top_k.pop();
                    top_k.push(neighbor);
                }
            }
        }

        let hits = top_k
            .into_iter()
            .filter(|neighbor| neighbor.is_fraud)
            .count();
        Ok(hits as i8)
    }
}

fn default_vector_source_path() -> String {
    std::env::var("LOAD_SCORE_FILE").unwrap_or_else(|_| "/app/resources/references.bin".to_string())
}

pub fn load_store_from_path(path: &str) -> Result<VectorStore, VectorStoreError> {
    VectorStore::from_mmap(path)
}

static STORE: OnceLock<Result<Arc<VectorStore>, VectorStoreError>> = OnceLock::new();

pub fn get_store() -> Result<&'static Arc<VectorStore>, VectorStoreError> {
    let source = default_vector_source_path();
    let initialized = STORE.get_or_init(|| {
        if !Path::new(&source).exists() {
            return Err(VectorStoreError::StoreUnavailable(format!(
                "vector source not found at {source}"
            )));
        }
        load_store_from_path(&source).map(Arc::new)
    });

    match initialized {
        Ok(store) => Ok(store),
        Err(error) => Err(VectorStoreError::StoreUnavailable(error.to_string())),
    }
}
