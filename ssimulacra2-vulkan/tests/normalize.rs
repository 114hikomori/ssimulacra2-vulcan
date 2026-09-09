// `normalize` subcommand contract (ROUND9, sibling-engine request): byte-exact
// rewrite of 8-bit images to plain truecolor PNG (IHDR/IDAT/IEND only). The
// engine's future drop-in GPU batch path depends on ALL of these properties.
use std::path::Path;
use std::process::Command;

fn run(args: &[&str]) -> (bool, String, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ssimulacra2-vulkan"))
        .current_dir("..")
        .args(args)
        .output()
        .expect("run cli");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn tmpdir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!(
        "s2v_norm_{}_{}_{}",
        tag,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Walk PNG chunks: returns (type, len) after the signature, and checks IHDR.
fn png_chunks(path: &Path) -> (Vec<(String, u32)>, u8, u8, u32, u32) {
    let b = std::fs::read(path).unwrap();
    assert_eq!(&b[0..8], b"\x89PNG\r\n\x1a\n", "signature");
    let mut pos = 8;
    let mut chunks = Vec::new();
    let mut depth = 0;
    let mut ctype = 0;
    let mut w = 0;
    let mut h = 0;
    while pos + 8 <= b.len() {
        let len = u32::from_be_bytes(b[pos..pos + 4].try_into().unwrap());
        let name = String::from_utf8_lossy(&b[pos + 4..pos + 8]).to_string();
        if name == "IHDR" {
            w = u32::from_be_bytes(b[pos + 8..pos + 12].try_into().unwrap());
            h = u32::from_be_bytes(b[pos + 12..pos + 16].try_into().unwrap());
            depth = b[pos + 16];
            ctype = b[pos + 17];
        }
        chunks.push((name, len));
        pos += 12 + len as usize;
    }
    (chunks, depth, ctype, w, h)
}

#[test]
fn normalized_output_is_truecolor8_and_chunks_only() {
    let d = tmpdir("shape");
    let (ok, _, e) = run(&[
        "normalize",
        "--out",
        d.to_str().unwrap(),
        "tests/fixtures/big_orig.png",
        "tests/fixtures/gray_orig.png",
    ]);
    assert!(ok, "normalize failed: {e}");
    let (chunks, depth, ctype, w, h) = png_chunks(&d.join("big_orig.png"));
    assert_eq!(depth, 8, "bit depth");
    assert_eq!(ctype, 2, "color type must be truecolor RGB");
    assert_eq!((w, h), (2048, 2048), "IHDR dims");
    let names: Vec<&str> = chunks.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(&names[0], &"IHDR", "first chunk");
    assert_eq!(names.last().unwrap(), &"IEND", "last chunk");
    assert!(
        names.iter().all(|n| matches!(*n, "IHDR" | "IDAT" | "IEND")),
        "extra chunks: {names:?}"
    );
    // gray input must come out as RGB (same dims, type 2) - gray was L-mode.
    let g = png_chunks(&d.join("gray_orig.png"));
    assert_eq!(g.2, 2, "gray must be expanded to truecolor");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn normalize_is_byte_deterministic() {
    let a = tmpdir("det_a");
    let b = tmpdir("det_b");
    for d in [&a, &b] {
        let (ok, _, e) = run(&[
            "normalize",
            "--out",
            d.to_str().unwrap(),
            "tests/fixtures/noise_orig.png",
        ]);
        assert!(ok, "{e}");
    }
    assert_eq!(
        std::fs::read(a.join("noise_orig.png")).unwrap(),
        std::fs::read(b.join("noise_orig.png")).unwrap(),
        "re-run must produce byte-identical files (cache-key stable)"
    );
    let _ = std::fs::remove_dir_all(&a);
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn score_is_neutral_under_normalization_all_paths() {
    // The engine's A/B premise: normalizing inputs must not move a single
    // score digit on any path (CPU-routed small, GPU large, and batch).
    let d = tmpdir("neutral");
    for pair in ["photo", "gray", "big"] {
        let o = format!("tests/fixtures/{pair}_orig.png");
        let v = format!("tests/fixtures/{pair}_dist.png");
        let (ok, _, e) = run(&["normalize", "--out", d.to_str().unwrap(), &o, &v]);
        assert!(ok, "normalize {pair}: {e}");
        let no = d.join(format!("{pair}_orig.png"));
        let nv = d.join(format!("{pair}_dist.png"));
        let raw = run(&[&o, &v]);
        let norm = run(&[no.to_str().unwrap(), nv.to_str().unwrap()]);
        assert!(raw.0 && norm.0, "{pair}: {:?}", (raw.2, norm.2));
        assert_eq!(raw.1, norm.1, "{pair}: score moved under normalize");
        let rawg = run(&["--gpu", &o, &v]);
        let normg = run(&["--gpu", no.to_str().unwrap(), nv.to_str().unwrap()]);
        assert_eq!(rawg.1, normg.1, "{pair}: GPU score moved under normalize");
    }
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn batch_score_is_neutral_under_normalization() {
    let d = tmpdir("neutral_batch");
    let vars_raw = tmpdir("neutral_batch_vraw");
    let vars_norm = tmpdir("neutral_batch_vnorm");
    std::fs::copy("../tests/fixtures/photo_dist.png", vars_raw.join("v1.png")).unwrap();
    let (ok, _, e) = run(&[
        "normalize",
        "--out",
        d.to_str().unwrap(),
        "tests/fixtures/photo_orig.png",
        "tests/fixtures/photo_dist.png",
    ]);
    assert!(ok, "{e}");
    std::fs::copy(d.join("photo_dist.png"), vars_norm.join("v1.png")).unwrap();
    let raw = run(&[
        "score-many",
        "--orig",
        "tests/fixtures/photo_orig.png",
        "--vars",
        vars_raw.to_str().unwrap(),
    ]);
    let norm = run(&[
        "score-many",
        "--orig",
        d.join("photo_orig.png").to_str().unwrap(),
        "--vars",
        vars_norm.to_str().unwrap(),
    ]);
    assert!(raw.0 && norm.0, "{:?}", (raw.2, norm.2));
    // same variant basename in both dirs -> identical "v1.png <score>" lines
    assert_eq!(raw.1, norm.1, "batch score moved under normalize");
    for p in [&d, &vars_raw, &vars_norm] {
        let _ = std::fs::remove_dir_all(p);
    }
}

#[test]
fn alpha_inputs_are_rejected_not_flattened() {
    let d = tmpdir("alpha");
    let (ok, _, e) = run(&[
        "normalize",
        "--out",
        d.to_str().unwrap(),
        "tests/fixtures/alpha_orig.png",
    ]);
    assert!(!ok, "real transparency must be rejected");
    assert!(e.contains("per-pair"), "message must route alpha: {e}");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn fully_opaque_rgba_is_stripped_score_neutral_and_clean() {
    // The sibling engine's corpus: RGBA originals with A=255 everywhere.
    // Stripping a zero-information channel is lossless - proven end to end
    // by score equality against the raw per-pair pass (which blends a no-op
    // worst-of-bg at alpha=1.0 and must land on the same %.8f).
    let d = tmpdir("opaque");
    let (ok, _, e) = run(&[
        "normalize",
        "--out",
        d.to_str().unwrap(),
        "tests/fixtures/opaque_orig.png",
        "tests/fixtures/opaque_dist.png",
    ]);
    assert!(ok, "opaque RGBA must be accepted: {e}");
    let raw = run(&["tests/fixtures/opaque_orig.png", "tests/fixtures/opaque_dist.png"]);
    let norm = run(&[
        d.join("opaque_orig.png").to_str().unwrap(),
        d.join("opaque_dist.png").to_str().unwrap(),
    ]);
    assert!(raw.0 && norm.0, "{:?}", (raw.2, norm.2));
    assert_eq!(raw.1, norm.1, "opaque strip moved the score");
    // stripped output is plain truecolor with only structural chunks
    let (chunks, depth, ctype, w, h) = png_chunks(&d.join("opaque_orig.png"));
    assert_eq!((depth, ctype, w, h), (8, 2, 128, 96), "stripped header");
    let names: Vec<&str> = chunks.iter().map(|(n, _)| n.as_str()).collect();
    assert!(
        names.iter().all(|n| matches!(*n, "IHDR" | "IDAT" | "IEND")),
        "extra chunks: {names:?}"
    );
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn pixel_neutral_chunks_are_ingested_and_stripped_iccp_stays_fatal() {
    // gAMA/cHRM on input: accepted by normalize, and the OUTPUT is
    // byte-identical to normalizing the plain twin - the chunks never
    // reach the score. iCCP (a pixel-remap claim) stays fatal everywhere.
    let a = tmpdir("gama");
    let b = tmpdir("plain");
    let (ok, _, e) = run(&[
        "normalize",
        "--out",
        a.to_str().unwrap(),
        "tests/fixtures/gama_orig.png",
    ]);
    assert!(ok, "gAMA/cHRM must be ingested: {e}");
    let (ok, _, e) = run(&[
        "normalize",
        "--out",
        b.to_str().unwrap(),
        "tests/fixtures/photo_orig.png",
    ]);
    assert!(ok, "{e}");
    assert_eq!(
        std::fs::read(a.join("gama_orig.png")).unwrap(),
        std::fs::read(b.join("photo_orig.png")).unwrap(),
        "gAMA input must normalize to the exact bytes the plain input produces"
    );
    let (ok, _, e) = run(&[
        "normalize",
        "--out",
        a.to_str().unwrap(),
        "tests/fixtures/icc_orig.png",
    ]);
    assert!(!ok, "iCCP must stay fatal");
    assert!(e.contains("iCCP"), "unexpected message: {e}");
    // ...and the scoring path's documented strictness is unchanged:
    let (ok, _, _) = run(&["--gpu", "tests/fixtures/gama_orig.png", "tests/fixtures/gama_orig.png"]);
    assert!(!ok, "scoring path must still reject gAMA (normalize is the ingest door)");
    for p in [&a, &b] {
        let _ = std::fs::remove_dir_all(p);
    }
}

#[test]
fn basename_collisions_are_rejected() {
    let d = tmpdir("collide");
    let a = tmpdir("collide_a");
    let b = tmpdir("collide_b");
    let src = std::fs::read("../tests/fixtures/photo_orig.png").unwrap();
    // two different paths, same basename - silent overwrite would corrupt a cache
    std::fs::write(a.join("same.png"), &src).unwrap();
    std::fs::write(b.join("same.png"), &src).unwrap();
    let (ok, _, e) = run(&[
        "normalize",
        "--out",
        d.to_str().unwrap(),
        a.join("same.png").to_str().unwrap(),
        b.join("same.png").to_str().unwrap(),
    ]);
    assert!(!ok, "collision must fail");
    assert!(e.contains("collision"), "unexpected message: {e}");
    for p in [&d, &a, &b] {
        let _ = std::fs::remove_dir_all(p);
    }
}
