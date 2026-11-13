"""Verify the declared files of an extracted SDK delivery."""
import hashlib
from pathlib import Path


def verify(root):
    root = Path(root).resolve()
    count = 0
    for line in (root / 'SHA256SUMS').read_text().splitlines():
        expected, name = line.split('  ', 1)
        path = root / name
        if path.is_symlink() or not path.resolve().is_relative_to(root) or not path.is_file():
            raise ValueError(f'missing or invalid package file: {name}')
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != expected:
            raise ValueError(f'package file checksum mismatch: {name}')
        count += 1
    return count


if __name__ == '__main__':
    print(f'Package files verified: {verify(Path(__file__).parent)}')
