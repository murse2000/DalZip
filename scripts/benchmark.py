"""동일 ZIP을 DalZip과 시스템 unzip으로 해제하고 모든 파일의 SHA-256을 비교합니다."""
import hashlib
import json
import pathlib
import random
import shutil
import subprocess
import tempfile
import time
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
subprocess.run(["cargo", "build", "--release", "--manifest-path", str(ROOT / "src-tauri/Cargo.toml"), "--example", "benchmark"], check=True)
engine = ROOT / "src-tauri/target/release/examples/benchmark"
results = []
with tempfile.TemporaryDirectory(prefix="dalzip-bench-") as work:
    work = pathlib.Path(work)
    rng = random.Random(20260913)
    for kind, count, size in [("large-files", 32, 4 * 1024 * 1024), ("small-files", 3000, 4096)]:
        archive = work / f"{kind}.zip"
        expected = {}
        with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=1, allowZip64=True) as z:
            for i in range(count):
                data = rng.randbytes(size // 2) + (b"DalZip benchmark " * (size // 16 + 1))[:size // 2]
                name = f"folder-{i % 16}/file-{i}.bin"
                expected[name] = hashlib.sha256(data).hexdigest()
                z.writestr(name, data)
        for iteration in range(3):
            for tool in (["dalzip", "unzip"] if iteration % 2 == 0 else ["unzip", "dalzip"]):
                output = work / f"{kind}-{iteration}-{tool}"
                output.mkdir()
                start = time.perf_counter()
                if tool == "dalzip":
                    result = subprocess.check_output([str(engine), str(archive), str(output)], text=True)
                    root = pathlib.Path(result.split("\t")[0])
                else:
                    subprocess.run(["/usr/bin/unzip", "-q", str(archive), "-d", str(output)], check=True)
                    root = output
                elapsed = time.perf_counter() - start
                actual = {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest() for p in root.rglob("*") if p.is_file()}
                assert actual == expected, f"결과 불일치: {kind} {tool}"
                results.append(dict(dataset=kind, tool=tool, iteration=iteration + 1, files=count, bytes=count * size, seconds=round(elapsed, 4), mib_per_second=round(count * size / elapsed / 1024 ** 2, 1), sha256_match=True))
                shutil.rmtree(output)
(ROOT / "artifacts").mkdir(exist_ok=True)
(ROOT / "artifacts/benchmark.json").write_text(json.dumps(results, ensure_ascii=False, indent=2))
print(json.dumps(results, ensure_ascii=False, indent=2))
