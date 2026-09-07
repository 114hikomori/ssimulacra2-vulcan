// Reader + comparator for oracle dumps (format: oracle/README.md).
use std::path::Path;

pub struct Dump {
    pub dtype: u32, // 3=u32, 4=f32, 8=f64
    pub xsize: u32,
    pub ysize: u32,
    pub channels: u32,
    pub f32_data: Vec<f32>,
    pub f64_data: Vec<f64>,
    pub u32_data: Vec<u32>,
}

impl Dump {
    pub fn read(path: impl AsRef<Path>) -> Dump {
        let bytes = std::fs::read(path).expect("dump file");
        assert!(bytes.len() >= 48, "dump too short");
        let g32 = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
        let g64 = |o: usize| u64::from_le_bytes(bytes[o..o + 8].try_into().unwrap());
        assert_eq!(g32(0), 0x31443253, "bad magic (not S2D1)");
        let dtype = g32(4);
        let (xsize, ysize, channels) = (g32(8), g32(12), g32(16));
        let payload = g64(24) as usize;
        assert_eq!(48 + payload, bytes.len(), "payload size mismatch");
        let mut d = Dump {
            dtype,
            xsize,
            ysize,
            channels,
            f32_data: vec![],
            f64_data: vec![],
            u32_data: vec![],
        };
        match dtype {
            4 => {
                d.f32_data = bytes[48..]
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes(c.try_into().unwrap()))
                    .collect()
            }
            8 => {
                d.f64_data = bytes[48..]
                    .chunks_exact(8)
                    .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                    .collect()
            }
            3 => {
                d.u32_data = bytes[48..]
                    .chunks_exact(4)
                    .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                    .collect()
            }
            other => panic!("dtype {other} unsupported"),
        }
        d
    }
}

/// (max abs diff, worst index) for f32 slices of equal length.
pub fn max_abs_diff(a: &[f32], b: &[f32]) -> (f32, usize) {
    assert_eq!(a.len(), b.len(), "length mismatch {} vs {}", a.len(), b.len());
    let mut worst = (0f32, 0usize);
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        let d = (x - y).abs();
        if d.is_nan() || d > worst.0 {
            worst = (d, i);
        }
    }
    worst
}
