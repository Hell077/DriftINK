#!/usr/bin/env python3
"""Drop/recover a mock IPC server while connected to an isolated real Wayland session."""
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import time
if len(sys.argv)!=2:sys.exit('Usage: check-reconnect.py <nested-wayland-display>')
env=os.environ.copy();env['WAYLAND_DISPLAY']=sys.argv[1];env.pop('DRIFTWM_SOCKET',None)
state=json.loads(subprocess.check_output(['driftwm','msg','--json','state'],env=env))['Ok']['State']
assert [o['name'] for o in state['outputs']]==['winit']
binary=str(Path(__file__).resolve().parents[1]/'target/debug/driftink')
with tempfile.TemporaryDirectory(prefix='driftink-reconnect-') as directory:
    env['DRIFTINK_DOCUMENT']=str(Path(directory)/'canvas.json')
    env['DRIFTWM_SOCKET']=str(Path(directory)/'ipc.sock')
    server=socket.socket(socket.AF_UNIX);server.bind(env['DRIFTWM_SOCKET']);server.listen();server.settimeout(0.2)
    online=threading.Event();online.set();stop=threading.Event();subscribed=threading.Event()
    def serve():
        while not stop.is_set():
            try:client,_=server.accept()
            except socket.timeout:continue
            with client:
                if not online.is_set():continue
                request=client.makefile().readline().strip()
                if request=='"Subscribe"':
                    client.sendall((json.dumps({'Ok':'Ok'})+'\n'+json.dumps({'State':state})+'\n').encode());subscribed.set()
                    while online.is_set() and not stop.is_set():time.sleep(0.03)
                    client.shutdown(socket.SHUT_RDWR)
                else:client.sendall((json.dumps({'Ok':{'State':state}})+'\n').encode())
    thread=threading.Thread(target=serve,daemon=True);thread.start()
    process=subprocess.Popen([binary,'draw'],env=env)
    def status():return subprocess.run([binary,'status'],env=env,capture_output=True,text=True)
    def wait_for(text):
        end=time.monotonic()+8
        while time.monotonic()<end:
            assert process.poll() is None
            result=status()
            if result.returncode==0 and text in result.stdout:return result.stdout
            time.sleep(0.1)
        raise AssertionError(f'Missing status: {text}')
    try:
        wait_for('drawing');assert subscribed.wait(3)
        online.clear();wait_for('IPC disconnected')
        disabled=status().stdout;assert disabled.startswith('navigation')
        assert subprocess.run([binary,'enable'],env=env,capture_output=True).returncode!=0
        online.set();wait_for('; connected;')
        assert status().stdout.startswith('navigation'), 'reconnect must not silently capture input'
        subprocess.run([binary,'enable'],env=env,check=True);wait_for('drawing')
        subprocess.run([binary,'quit'],env=env,check=True);assert process.wait(timeout=5)==0
        print('IPC loss disables capture; subscription reconnects; editing requires explicit enable: PASS')
        path=Path(env['DRIFTINK_DOCUMENT']);path.write_text('{broken')
        failed=subprocess.run([binary,'run'],env=env,capture_output=True)
        assert failed.returncode!=0 and path.read_text()=='{broken'
        print('Corrupt document is rejected and preserved: PASS')
    finally:
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        stop.set();online.clear();thread.join(timeout=2);server.close()
