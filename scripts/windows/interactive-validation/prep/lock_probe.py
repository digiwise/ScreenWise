import ctypes as C, os, json
from ctypes import wintypes as W
from datetime import datetime, timezone
class Level1(C.Structure):
    _fields_=[('SessionId',W.ULONG),('SessionState',C.c_int),('SessionFlags',W.LONG),('WinStationName',W.WCHAR*33),('UserName',W.WCHAR*21),('DomainName',W.WCHAR*18),('LogonTime',C.c_longlong),('ConnectTime',C.c_longlong),('DisconnectTime',C.c_longlong),('LastInputTime',C.c_longlong),('CurrentTime',C.c_longlong),('IncomingBytes',W.DWORD),('OutgoingBytes',W.DWORD),('IncomingFrames',W.DWORD),('OutgoingFrames',W.DWORD),('IncomingCompressedBytes',W.DWORD),('OutgoingCompressedBytes',W.DWORD)]
class Info(C.Structure):
    _fields_=[('Level',W.DWORD),('Data',Level1)]
k=C.WinDLL('kernel32',use_last_error=True)
k.ProcessIdToSessionId.argtypes=[W.DWORD,C.POINTER(W.DWORD)]
wts=C.WinDLL('wtsapi32',use_last_error=True)
wts.WTSQuerySessionInformationW.argtypes=[W.HANDLE,W.DWORD,C.c_int,C.POINTER(C.c_void_p),C.POINTER(W.DWORD)]
wts.WTSFreeMemory.argtypes=[C.c_void_p]
u=C.WinDLL('user32',use_last_error=True)
u.OpenInputDesktop.argtypes=[W.DWORD,W.BOOL,W.DWORD];u.OpenInputDesktop.restype=W.HANDLE
u.CloseDesktop.argtypes=[W.HANDLE]
u.GetUserObjectInformationW.argtypes=[W.HANDLE,C.c_int,C.c_void_p,W.DWORD,C.POINTER(W.DWORD)]
def probe():
    session=W.DWORD()
    if not k.ProcessIdToSessionId(os.getpid(),C.byref(session)):raise C.WinError(C.get_last_error())
    result={'utc':datetime.now(timezone.utc).isoformat(),'session':session.value}
    buf=C.c_void_p();size=W.DWORD()
    if not wts.WTSQuerySessionInformationW(None,session.value,25,C.byref(buf),C.byref(size)):
        result['wts_error']=C.get_last_error()
    else:
        try:
            if size.value<C.sizeof(Info):raise RuntimeError('Short WTS info buffer')
            info=C.cast(buf,C.POINTER(Info)).contents
            result.update(wts_level=info.Level,wts_session=info.Data.SessionId,wts_flags=info.Data.SessionFlags,wts_state=info.Data.SessionState)
        finally:wts.WTSFreeMemory(buf)
    for access in (0,1,0x100):
        C.set_last_error(0);h=u.OpenInputDesktop(0,False,access)
        point={'available':bool(h),'error':C.get_last_error() if not h else 0}
        if h:
            try:
                name=C.create_unicode_buffer(256);needed=W.DWORD()
                if u.GetUserObjectInformationW(h,2,name,C.sizeof(name),C.byref(needed)):point['desktop_name']=name.value
                else:point['name_error']=C.get_last_error()
            finally:u.CloseDesktop(h)
        result['desktop_access_'+str(access)]=point
    result['locked_corroborated']=result.get('wts_level')==1 and result.get('wts_session')==session.value and result.get('wts_flags')==0
    return result
if __name__=='__main__':print(json.dumps(probe()))
