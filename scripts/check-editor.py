#!/usr/bin/env python3
"""End-to-end drawing in installed driftwm, isolated Xvfb :95 (no desktop input).
Start the nested compositor using docs/testing.md before running this script.
"""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time
sys.dont_write_bytecode=True
from xvfb_input import Pointer

if len(sys.argv)!=2:
    sys.exit('Usage: check-editor.py <nested-wayland-display>')
env=os.environ.copy();env['WAYLAND_DISPLAY']=sys.argv[1];env.pop('DRIFTWM_SOCKET',None)
repo=Path(__file__).resolve().parents[1];binary=str(repo/'target/debug/driftink')
def compositor(*args):
    return subprocess.check_output(['driftwm','msg','--json',*args],env=env,text=True)
def state():return json.loads(compositor('state'))['Ok']['State']
assert [(o['name'],o['size']) for o in state()['outputs']]==[('winit',[1280,800])]
compositor('zoom','1');time.sleep(0.3);compositor('camera','0','0');time.sleep(0.3)
pointer=Pointer()
def command(cmd):return subprocess.check_output([binary,cmd],env=env,text=True).strip()
def ready(process):
    end=time.monotonic()+10
    while time.monotonic()<end:
        assert process.poll() is None, 'editor exited early'
        result=subprocess.run([binary,'status'],env=env,text=True,capture_output=True)
        if result.returncode==0:return result.stdout
        time.sleep(0.1)
    raise AssertionError('editor did not start')
def click_through(observer):
    pointer.click(640,400)
    assert select.select([observer.stdout],[],[],3)[0], 'normal native window did not receive click'
    assert observer.stdout.readline().strip()=='CLICK'

with tempfile.TemporaryDirectory(prefix='driftink-editor-') as directory:
    path=Path(directory)/'canvas.json';env['DRIFTINK_DOCUMENT']=str(path)
    log=open(Path(directory)/'app.log','w+')
    process=subprocess.Popen([binary,'draw'],env=env,stdout=log,stderr=log)
    observer=None
    try:
        assert 'drawing' in ready(process)
        duplicate=subprocess.run([binary,'run'],env=env,capture_output=True,text=True)
        assert duplicate.returncode!=0 and 'owns this document' in duplicate.stderr
        def document():return json.loads(path.read_text())
        def count():return len(document()['objects'])
        def tool(index):pointer.click(100,140+index*40)
        tool(1);pointer.drag((350,200),(600,350))
        tool(4);pointer.drag((650,200),(1000,350))
        tool(5);pointer.drag((350,430),(600,600))
        tool(6);pointer.drag((700,450),(1000,650))
        tool(2);pointer.drag((400,700),(600,700))
        tool(3);pointer.drag((1050,200),(1100,600))
        assert [o['tool'] for o in document()['objects']]==['Pen','Arrow','Rectangle','Ellipse','Marker','Line']
        assert document()['objects'][0]['points'][0]==[-290.0,200.0]
        assert document()['objects'][4]['color'][3]==90
        print('Pen, marker, line, arrow, rectangle, ellipse; world coordinates; autosave: PASS',flush=True)
        pointer.click(60,658);assert count()==5
        pointer.click(150,658);assert count()==6
        pointer.tap(47);pointer.drag((475,275),(575,375))
        assert document()['objects'][0]['points'][0]==[-190.0,100.0]
        pointer.key(29,True);pointer.tap(44);pointer.key(29,False)
        assert document()['objects'][0]['points'][0]==[-290.0,200.0]
        tool(7);pointer.click(475,275);assert count()==5
        command('undo');assert count()==6
        print('Sidebar undo/redo, selection move, Ctrl+Z and eraser: PASS',flush=True)
        tool(1);pointer.move(900,700);pointer.button(True);pointer.move(1100,700);pointer.tap(1);pointer.button(False)
        assert count()==6 and 'drawing' in command('status')
        pointer.tap(1);time.sleep(0.3);assert 'navigation' in command('status')
        observer=subprocess.Popen([str(repo/'target/debug/examples/input_probe')],env=env,stdout=subprocess.PIPE,text=True)
        time.sleep(0.7);click_through(observer)
        print('Escape cancels gesture, then exits; normal native window receives click: PASS',flush=True)
        compositor('action','toggle-fullscreen');time.sleep(0.6)
        assert state()['fullscreen'], 'native test window did not enter fullscreen'
        command('enable');time.sleep(0.4)
        click_through(observer)
        compositor('action','toggle-fullscreen');time.sleep(0.5)
        command('disable');observer.terminate();observer.wait(timeout=5);observer=None
        print('Fullscreen hides drawing input and restores safely: PASS',flush=True)
        saved=document()
        compositor('zoom','0.5');time.sleep(0.8);compositor('camera','100','50');time.sleep(0.6)
        assert document()==saved
        artifacts=Path(os.environ.get('DRIFTINK_TEST_ARTIFACTS',directory));artifacts.mkdir(parents=True,exist_ok=True)
        subprocess.run(['grim',str(artifacts/'navigation.png')],env=env,check=True)
        command('enable');time.sleep(0.3)
        subprocess.run(['grim',str(artifacts/'sidebar.png')],env=env,check=True)
        command('quit');assert process.wait(timeout=5)==0
        process=subprocess.Popen([binary,'run'],env=env,stdout=log,stderr=log)
        assert 'navigation; 6 objects' in ready(process) and document()==saved
        before=Path(f'/proc/{process.pid}/stat').read_text().split()
        time.sleep(2)
        after=Path(f'/proc/{process.pid}/stat').read_text().split()
        ticks=sum(map(int,after[13:15]))-sum(map(int,before[13:15]))
        print(f'Pan/zoom preserve geometry; restart restores 6 objects; idle CPU ticks/2s={ticks}: PASS',flush=True)
        command('quit');assert process.wait(timeout=5)==0
    except BaseException:
        log.flush();log.seek(0);print(log.read(),file=sys.stderr);raise
    finally:
        if observer and observer.poll() is None:observer.terminate();observer.wait(timeout=5)
        if process.poll() is None:process.terminate();process.wait(timeout=5)
        pointer.close();log.close()
        compositor('zoom','1');time.sleep(0.3);compositor('camera','0','0')
