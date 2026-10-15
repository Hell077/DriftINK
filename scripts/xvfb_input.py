"""XTest driver restricted to a disposable :95 X server with one compositor window."""
import ctypes as c
import time

class Pointer:
    def __init__(self):
        self.x=c.CDLL('libX11.so.6');self.xt=c.CDLL('libXtst.so.6')
        x=self.x;xt=self.xt
        x.XOpenDisplay.argtypes=[c.c_char_p];x.XOpenDisplay.restype=c.c_void_p
        x.XDefaultRootWindow.argtypes=[c.c_void_p];x.XDefaultRootWindow.restype=c.c_ulong
        x.XQueryTree.argtypes=[c.c_void_p,c.c_ulong,c.POINTER(c.c_ulong),c.POINTER(c.c_ulong),c.POINTER(c.POINTER(c.c_ulong)),c.POINTER(c.c_uint)]
        x.XSetInputFocus.argtypes=[c.c_void_p,c.c_ulong,c.c_int,c.c_ulong]
        x.XFlush.argtypes=[c.c_void_p];x.XCloseDisplay.argtypes=[c.c_void_p];x.XFree.argtypes=[c.c_void_p]
        xt.XTestFakeMotionEvent.argtypes=[c.c_void_p,c.c_int,c.c_int,c.c_int,c.c_ulong]
        xt.XTestFakeButtonEvent.argtypes=[c.c_void_p,c.c_uint,c.c_int,c.c_ulong]
        xt.XTestFakeKeyEvent.argtypes=[c.c_void_p,c.c_uint,c.c_int,c.c_ulong]
        self.d=x.XOpenDisplay(b':95');assert self.d, 'Start isolated Xvfb :95 first'
        root=x.XDefaultRootWindow(self.d)
        a=c.c_ulong();b=c.c_ulong();children=c.POINTER(c.c_ulong)();n=c.c_uint()
        x.XQueryTree(self.d,root,c.byref(a),c.byref(b),c.byref(children),c.byref(n))
        assert n.value==1,'Expected only the nested compositor in Xvfb'
        self.window=children[0];x.XSetInputFocus(self.d,self.window,1,0);x.XFree(children);x.XFlush(self.d)
    def resize(self,width,height):
        self.x.XResizeWindow.argtypes=[c.c_void_p,c.c_ulong,c.c_uint,c.c_uint]
        self.x.XResizeWindow(self.d,self.window,width,height);self.x.XFlush(self.d);time.sleep(0.4)
    def move(self,x,y):
        self.xt.XTestFakeMotionEvent(self.d,0,int(x),int(y),0);self.x.XFlush(self.d)
        time.sleep(0.025)
    def button(self,pressed,button=1):
        self.xt.XTestFakeButtonEvent(self.d,button,int(pressed),0);self.x.XFlush(self.d);time.sleep(0.035)
    def click(self,x,y):
        self.move(x,y);self.button(True);self.button(False);time.sleep(0.1)
    def drag(self,start,end):
        self.move(*start);self.button(True)
        for i in range(1,17):self.move(start[0]+(end[0]-start[0])*i/16,start[1]+(end[1]-start[1])*i/16)
        self.button(False);time.sleep(0.15)
    def key(self,evdev,pressed):
        self.xt.XTestFakeKeyEvent(self.d,evdev+8,int(pressed),0);self.x.XFlush(self.d);time.sleep(0.04)
    def tap(self,evdev):self.key(evdev,True);self.key(evdev,False)
    def close(self):self.x.XCloseDisplay(self.d)
