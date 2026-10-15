#!/usr/bin/env python3
"""Build and install DriftInk for the current user, or remove installed files.
Never changes driftwm configuration or deletes drawings.
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def absolute(value):
    path = Path(value).expanduser()
    if not path.is_absolute() or any(ord(ch) < 32 for ch in str(path)):
        raise argparse.ArgumentTypeError('use an absolute path without control characters')
    return path


def desktop_value(value):
    return str(value).replace('\\', '\\\\')


def desktop_command(path, command):
    # Desktop Entry quoting is distinct from shell quoting; no shell is invoked.
    word = str(path).replace('%', '%%')
    for char in ['\\', '"', '`', '$']:
        word = word.replace(char, '\\' + char)
    return desktop_value('"' + word + '" ' + command)


def atomic_write(path, content, mode):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix='.driftink-', delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(content)
            stream.flush()
            os.fsync(stream.fileno())
        temporary.chmod(mode)
        os.replace(temporary, path)
    finally:
        if temporary:
            temporary.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--prefix', type=absolute, default=Path.home() / '.local',
                        help='installation prefix (default: ~/.local)')
    parser.add_argument('--data-dir', type=absolute,
                        help='desktop/icon directory (default: XDG_DATA_HOME or PREFIX/share)')
    parser.add_argument('--binary', type=absolute,
                        help='install a prebuilt executable instead of building with Cargo')
    parser.add_argument('--uninstall', action='store_true', help='remove executable, launcher and icon; keep drawings')
    args = parser.parse_args()
    data = args.data_dir or absolute(os.environ.get('XDG_DATA_HOME', str(args.prefix / 'share')))
    executable = args.prefix / 'bin/driftink'
    icon = data / 'icons/hicolor/scalable/apps/driftink.svg'
    launcher = data / 'applications/driftink.desktop'
    if args.uninstall:
        for path in [executable, launcher, icon]:
            path.unlink(missing_ok=True)
            print(f'Removed: {path}')
        print('Drawings and driftwm configuration have been preserved.')
    else:
        root = Path(__file__).resolve().parents[1]
        if args.binary:
            binary = args.binary
        else:
            subprocess.run(['cargo', 'build', '--release', '--locked'], cwd=root, check=True)
            import json
            metadata = subprocess.check_output(['cargo', 'metadata', '--no-deps', '--format-version=1'], cwd=root)
            binary = Path(json.loads(metadata)['target_directory']) / 'release/driftink'
        if not binary.is_file() or not os.access(binary, os.X_OK):
            parser.error(f'not an executable file: {binary}')
        desktop = f'''[Desktop Entry]
Type=Application
Name=DriftInk
Comment=Draw on the driftwm canvas
Comment[ru]=Рисование на холсте driftwm
Exec={desktop_command(executable, 'draw')}
Icon={desktop_value(icon)}
Terminal=false
StartupNotify=false
Categories=Graphics;2DGraphics;VectorGraphics;
Keywords=draw;canvas;pen;arrow;driftwm;
Actions=Toggle;Quit;

[Desktop Action Toggle]
Name=Toggle drawing mode
Name[ru]=Переключить режим рисования
Exec={desktop_command(executable, 'toggle')}

[Desktop Action Quit]
Name=Quit DriftInk
Name[ru]=Закрыть DriftInk
Exec={desktop_command(executable, 'quit')}
'''
        # Read all source assets before replacing an existing installation.
        files = [(executable, binary.read_bytes(), 0o755),
                 (icon, (root / 'packaging/driftink.svg').read_bytes(), 0o644),
                 (launcher, desktop.encode(), 0o644)]
        for path, content, mode in files:
            atomic_write(path, content, mode)
            print(f'Installed: {path}')
        print(f'Launch from the application menu, or run: {executable} draw')
        print('No autostart or keyboard shortcut was installed. See README.md.')
    if shutil.which('update-desktop-database') and launcher.parent.exists():
        subprocess.run(['update-desktop-database', str(launcher.parent)], check=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(f'Installation failed: {error}') from error
