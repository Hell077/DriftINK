#!/usr/bin/env python3
"""Measure anchoring in an isolated driftwm session. Never targets wayland-1 implicitly.
Requires grim and ImageMagick. Pass the nested WAYLAND_DISPLAY explicitly.
"""
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time

if len(sys.argv) != 2:
    sys.exit('Usage: scripts/check-poc.py <nested-wayland-display>')
env = os.environ.copy()
env['WAYLAND_DISPLAY'] = sys.argv[1]
env.pop('DRIFTWM_SOCKET', None)
sockpath = Path(env['XDG_RUNTIME_DIR']) / 'driftwm' / f'ipc-{sys.argv[1]}.sock'

def request(value):
    with socket.socket(socket.AF_UNIX) as client:
        client.connect(str(sockpath))
        client.sendall((json.dumps(value)+'\n').encode())
        reply = json.loads(client.makefile().readline())
        if 'Err' in reply:
            raise RuntimeError(reply['Err'])
        return reply['Ok']

state = request('State')['State']
if [o['name'] for o in state['outputs']] != ['winit']:
    sys.exit('Refusing camera changes: expected a single nested winit output')
original = state['outputs'][0]
if original['size'] != [1280, 800]:
    sys.exit('Expected 1280x800 nested output for reference pixel bounds')
root = Path(__file__).resolve().parents[1]
try:
    with tempfile.TemporaryDirectory(prefix='driftink-check-') as directory:
        for strategy in ['canvas', 'overlay']:
            request({'Camera': [0, 0]})
            request({'Zoom': 1.0})
            process = subprocess.Popen([str(root/'target/debug/driftink'), 'poc', strategy, '60'], env=env)
            try:
                for camera, zoom, expected in [([0,0],1.0,'300x220+490+290'), ([100,50],0.5,'150x110+515+370')]:
                    request({'Zoom': zoom})
                    time.sleep(4)
                    request({'Camera': camera})
                    deadline = time.monotonic() + 12
                    path = str(Path(directory)/f'{strategy}-{zoom}.png')
                    while True:
                        time.sleep(0.4)
                        if process.poll() is not None:
                            raise RuntimeError(f'{strategy} exited early: {process.returncode}')
                        subprocess.run(['grim',path],env=env,check=True)
                        bounds = subprocess.check_output(['magick',path,'-alpha','off','-transparent','#cba6f7','-alpha','extract','-negate','-format','%@','info:'],text=True)
                        if bounds == expected:
                            break
                        if time.monotonic() > deadline:
                            raise AssertionError((strategy,camera,zoom,bounds,expected,request('State')))
                    print(f'{strategy}: camera={camera}, zoom={zoom}, rectangle={bounds}: PASS',flush=True)
                if strategy == 'canvas':
                    layers = request('State')['State']['canvas_layers']
                    assert any(x['position']==[0,0] and x['size']==[320,240] for x in layers)
            finally:
                process.terminate()
                process.wait(timeout=5)
                time.sleep(0.2)
finally:
    request({'Zoom': original['zoom']})
    time.sleep(4)
    request({'Camera': original['camera']})
