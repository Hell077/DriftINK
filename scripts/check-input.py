#!/usr/bin/env python3
"""Native input test in a disposable Xvfb + nested driftwm session.
Usage: check-input.py wayland-2 :95
Only accepts a single winit output. Never injects into the desktop X server.
"""
import ctypes as c
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import time

if len(sys.argv) != 3 or sys.argv[2] != ':95':
    sys.exit('Usage: check-input.py <nested-wayland-display> :95 (isolated Xvfb)')
env=os.environ.copy()
env['WAYLAND_DISPLAY']=sys.argv[1]
env.pop('DRIFTWM_SOCKET',None)
state=json.loads(subprocess.check_output(['driftwm','msg','--json','state'],env=env))['Ok']['State']
assert [o['name'] for o in state['outputs']] == ['winit']
x=c.CDLL('libX11.so.6'); xt=c.CDLL('libXtst.so.6')
x.XOpenDisplay.argtypes=[c.c_char_p];x.XOpenDisplay.restype=c.c_void_p
x.XDefaultRootWindow.argtypes=[c.c_void_p];x.XDefaultRootWindow.restype=c.c_ulong
x.XQueryTree.argtypes=[c.c_void_p,c.c_ulong,c.POINTER(c.c_ulong),c.POINTER(c.c_ulong),c.POINTER(c.POINTER(c.c_ulong)),c.POINTER(c.c_uint)]
x.XSetInputFocus.argtypes=[c.c_void_p,c.c_ulong,c.c_int,c.c_ulong]
x.XFlush.argtypes=[c.c_void_p];x.XCloseDisplay.argtypes=[c.c_void_p];x.XFree.argtypes=[c.c_void_p]
xt.XTestFakeMotionEvent.argtypes=[c.c_void_p,c.c_int,c.c_int,c.c_int,c.c_ulong]
xt.XTestFakeButtonEvent.argtypes=[c.c_void_p,c.c_uint,c.c_int,c.c_ulong]
d=x.XOpenDisplay(sys.argv[2].encode());assert d
root=x.XDefaultRootWindow(d)
a=c.c_ulong();b=c.c_ulong();children=c.POINTER(c.c_ulong)();n=c.c_uint()
x.XQueryTree(d,root,c.byref(a),c.byref(b),c.byref(children),c.byref(n))
assert n.value==1, 'Expected only the nested compositor in Xvfb'
x.XSetInputFocus(d,children[0],1,0);x.XFree(children);x.XFlush(d)
repo=Path(__file__).resolve().parents[1]
window=subprocess.Popen([str(repo/'target/debug/examples/input_probe')],env=env,stdout=subprocess.PIPE,text=True)
try:
    time.sleep(1)
    for strategy in ['overlay','canvas']:
        probe=subprocess.Popen([str(repo/'target/debug/driftink'),'poc',strategy,'8'],env=env)
        try:
            time.sleep(0.5)
            xt.XTestFakeMotionEvent(d,0,640,400,0);x.XFlush(d);time.sleep(0.2)
            xt.XTestFakeButtonEvent(d,1,1,0);xt.XTestFakeButtonEvent(d,1,0,0);x.XFlush(d)
            assert select.select([window.stdout],[],[],3)[0], f'{strategy}: click did not reach normal window'
            assert window.stdout.readline().strip()=='CLICK'
            print(f'{strategy}: click passed to native window: PASS',flush=True)
        finally:
            probe.terminate();probe.wait(timeout=5)
finally:
    window.terminate();window.wait(timeout=5);x.XCloseDisplay(d)
