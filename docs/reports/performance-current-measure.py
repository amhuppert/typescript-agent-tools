"""Measure the final tsparse binary on the frozen Command Center copy.

Run from this repository on macOS:
    python3 docs/reports/performance-current-measure.py operations
    python3 docs/reports/performance-current-measure.py files
    python3 docs/reports/performance-current-measure.py refresh
    python3 docs/reports/performance-current-measure.py direct

Requires the binary and corpus paths below from the Performance session.
Results append under .cc/temp/current-report/. Run phases sequentially.
The operations phase skips complete cases already present in its result file.
The refresh phase restores the single edited corpus file in a finally block.
File application only changes scratch copies. No earlier binary is measured.
The HTML report also uses separate service-control and stack-profile probes.
"""
import contextlib, hashlib, importlib.util, json, os, pathlib, re, subprocess, sys, tempfile, threading, time
ROOT=next(p for p in pathlib.Path(__file__).resolve().parents if (p/'Cargo.toml').is_file())
OUT=ROOT/'.cc/temp/current-report'
OUT.mkdir(parents=True,exist_ok=True)
CC=ROOT/'.cc/temp/reuse-perf/command-center'
BIN=ROOT/'.cc/temp/reuse-perf/final/tsparse'
assert hashlib.sha256(BIN.read_bytes()).hexdigest() == '61eafb2b36ba5c761bd7cd6afa539cc55106238def6745f07bfdb7f7a7f519f3', 'wrong binary'
"""Read a process's footprint and CPU time on macOS through proc_pid_rusage."""
import ctypes, os

_libc = ctypes.CDLL("/usr/lib/libSystem.dylib", use_errno=True)
_FIELDS = [
    "user_time", "system_time", "pkg_idle_wkups", "interrupt_wkups", "pageins",
    "wired_size", "resident_size", "phys_footprint", "proc_start_abstime",
    "proc_exit_abstime", "child_user_time", "child_system_time",
    "child_pkg_idle_wkups", "child_interrupt_wkups", "child_pageins",
    "child_elapsed_abstime", "diskio_bytesread", "diskio_byteswritten",
    "cpu_time_qos_default", "cpu_time_qos_maintenance", "cpu_time_qos_background",
    "cpu_time_qos_utility", "cpu_time_qos_legacy", "cpu_time_qos_user_initiated",
    "cpu_time_qos_user_interactive", "billed_system_time", "serviced_system_time",
    "logical_writes", "lifetime_max_phys_footprint", "instructions", "cycles",
    "billed_energy", "serviced_energy", "interval_max_phys_footprint", "runnable_time",
]


class RusageInfoV4(ctypes.Structure):
    _fields_ = [("uuid", ctypes.c_uint8 * 16)] + [(f, ctypes.c_uint64) for f in _FIELDS]


_libc.proc_pid_rusage.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.POINTER(RusageInfoV4)]


class _Timebase(ctypes.Structure):
    _fields_ = [("numer", ctypes.c_uint32), ("denom", ctypes.c_uint32)]


_tb = _Timebase()
_libc.mach_timebase_info(ctypes.byref(_tb))
NS_PER_TICK = _tb.numer / _tb.denom


def rusage(pid):
    info = RusageInfoV4()
    if _libc.proc_pid_rusage(pid, 4, ctypes.byref(info)) != 0:
        raise OSError(ctypes.get_errno(), os.strerror(ctypes.get_errno()))
    return {f: getattr(info, f) for f in _FIELDS}


def reset_interval(pid):
    fn = getattr(_libc, "proc_reset_footprint_interval", None)
    if fn is None:
        return False
    fn.argtypes = [ctypes.c_int]
    return fn(pid) == 0



SMALL='src/features/tickets/components/TicketChildSummary.tsx'
LARGE='src/lib/specs/review-service.ts'
P=['--project','tsconfig.json']
FRAG=OUT/'body.ts';FRAG.write_text('{ return null; }\n')
INSERT=OUT/'insert.ts';INSERT.write_text('export const reportProbe = 1;\n')
BATCH=OUT/'batch.json'; BATCH.write_text(json.dumps({'edits':[{'operation':'replace_body','name':'TicketChildSummary','file':SMALL,'from':str(FRAG)},{'operation':'insert_after','name':'snapshotOrNull','file':LARGE,'from':str(INSERT)}]}))
CASES={
 'outline':['--outline',*P,SMALL],
 'outline_large':['--outline',*P,LARGE,'--depth','all','--json'],
 'show':['--show',*P,'--name','TicketChildSummary',SMALL],
 'references':['--references',*P,'--name','getTicketService'],
 'type':['--type',*P,'--at','src/lib/tickets/service-factory.ts:201:17'],
 'type_expand':['--type',*P,'--at','src/lib/tickets/service-factory.ts:201:17','--expand'],
 'replace_body':['--replace-body',*P,'--name','TicketChildSummary','--from',str(FRAG),SMALL],
 'insert_after':['--insert-after',*P,'--name','TicketChildSummary','--from',str(INSERT),SMALL],
 'edit_batch':['--edit-batch',str(BATCH),*P],
 'member':['--references',*P,'--name','AgentTaskRunner.run'],
}
gold={}
def emit(row):
 with (OUT/(sys.argv[1]+'.jsonl')).open('a') as f:f.write(json.dumps(row)+'\n')
 print(json.dumps({k:v for k,v in row.items() if k not in ['trace','args','stderr']}),flush=True)
def therm():return subprocess.check_output(['pmset','-g','therm'],text=True)
def run(args,env=None,pid=None,trace=False,expected=0):
 before=rusage(pid) if pid else None
 reset=reset_interval(pid) if pid else False
 points=[];stop=threading.Event();start=time.perf_counter()
 def sample():
  while not stop.is_set():
   r=rusage(pid);points.append([time.perf_counter()-start,r['phys_footprint']]);stop.wait(.02)
 sampler=threading.Thread(target=sample) if pid and trace else None
 if sampler:sampler.start()
 tf=OUT/'last.time'
 try:
  p=subprocess.run(['/usr/bin/time','-l','-o',str(tf),str(BIN),*args],cwd=CC,env=env,capture_output=True,timeout=180)
  wall=time.perf_counter()-start
 finally:
  stop.set()
  if sampler:sampler.join()
 assert p.returncode==expected,(args,p.returncode,p.stderr[:1000],p.stdout[:1000])
 after=rusage(pid) if pid else None
 row={'wall_s':wall,'exit':p.returncode,'bytes':len(p.stdout),'sha256':hashlib.sha256(p.stdout).hexdigest(),'args':args}
 text=tf.read_text();m=re.search(r'([\d.]+) real\s+([\d.]+) user\s+([\d.]+) sys',text)
 row['client_cpu_s']=float(m[2])+float(m[3])
 for label,key in [('peak memory footprint','client_peak_bytes'),('instructions retired','client_instructions')]:
  row[key]=int(re.search(r'(\d+)\s+'+label,text)[1])
 if pid:
  row.update(service_cpu_s=(after['user_time']+after['system_time']-before['user_time']-before['system_time'])/1e9,service_before_bytes=before['phys_footprint'],service_after_bytes=after['phys_footprint'],service_peak_bytes=after['interval_max_phys_footprint'] if reset else None,interval_reset=reset,service_instructions=after['instructions']-before['instructions'])
  if trace:row['trace']=points
 return row,p.stdout
@contextlib.contextmanager
def service():
 with tempfile.TemporaryDirectory(prefix='tsp-report-',dir='/tmp') as rt:
  env=dict(os.environ,TSPARSE_SERVICE_DIR=rt,TSPARSE_SERVICE_IDLE_SECS='600')
  err=open(OUT/'service.err','wb')
  server=subprocess.Popen([str(BIN),'--references-serve',*P],cwd=CC,env=env,stdout=subprocess.DEVNULL,stderr=err)
  def status():return json.loads(subprocess.check_output([str(BIN),'--references-status',*P],cwd=CC,env=env))
  try:
   deadline=time.monotonic()+30
   while not (st:=status()).get('running'):
    assert server.poll() is None;assert time.monotonic()<deadline;time.sleep(.03)
   assert st['generation']==0
   yield env,st['pid'],status
  finally:
   if server.poll() is None:subprocess.run([str(BIN),'--references-stop',*P],cwd=CC,env=env,check=True,stdout=subprocess.DEVNULL)
   assert server.wait(timeout=30)==0
   err.close()
def check(name,data):
 if name in gold:assert gold[name]==data,name
 else:
  recorded=[json.loads(x) for x in (OUT/'operations.jsonl').read_text().splitlines()] if (OUT/'operations.jsonl').exists() else []
  for r in recorded:
   if r.get('kind')=='project' and r['case']==name: assert r['sha256']==hashlib.sha256(data).hexdigest(),name
  gold[name]=data
with (OUT/'host-conditions.jsonl').open('a') as f:f.write(json.dumps({'phase':sys.argv[1],'time':time.time(),'thermal':therm(),'load':os.getloadavg()})+'\n')
if sys.argv[1]=='operations':
 for round in range(2):
  names=list(CASES) if round==0 else list(reversed(CASES))
  for name in names:
   existing=[json.loads(x) for x in (OUT/'operations.jsonl').read_text().splitlines()] if (OUT/'operations.jsonl').exists() else []
   done=[r for r in existing if r.get('kind')=='project' and r['round']==round and r['case']==name]
   if done:
    assert len(done)==(3 if name=='member' else 6), 'incomplete case'
    continue
   emit({'kind':'condition','round':round,'case':name,'thermal':therm(),'load':os.getloadavg()})
   with service() as (env,pid,status):
    for index in range(3 if name=='member' else 6):
     row,data=run(CASES[name],env,pid,trace=index<2,expected=1 if name in ['member','type_expand'] else 0);check(name,data)
     assert status()['generation']==1
     emit(dict(kind='project',case=name,round=round,state='cold' if index==0 else 'warm',sample=index,**row))
    for i in range(3):
     row,data=run(['--references-status',*P],env);emit(dict(kind='control',case='status',round=round,state='warm',**row))
elif sys.argv[1]=='files':
 for size,path in [('small',SMALL),('large',LARGE)]:
  name='TicketChildSummary' if size=='small' else 'snapshotOrNull'
  for case,args in {'parse':[path],'check':['--check',path],'outline':['--outline',path],'show':['--show','--name',name,path],'replace_body':['--replace-body','--name',name,'--from',str(FRAG),path],'insert_after':['--insert-after','--name',name,'--from',str(INSERT),path]}.items():
   for i in range(6):
    row,data=run(args);check(f'{case}-{size}',data);emit(dict(kind='file',case=case,size=size,input_bytes=(CC/path).stat().st_size,state='first' if i==0 else 'repeat',sample=i,**row))
 for i in range(6):
  row,data=run(['--edit-batch',str(BATCH)]);check('batch',data);emit(dict(kind='file',case='edit_batch',size='two files',state='first' if i==0 else 'repeat',sample=i,**row))
 for size,path in [('small',SMALL),('large',LARGE)]:
  original=(CC/path).read_bytes();copy=OUT/f'apply-{size}.tsx';plan=OUT/f'apply-{size}.json';name='TicketChildSummary' if size=='small' else 'snapshotOrNull'
  for i in range(6):
   copy.write_bytes(original);plan.unlink(missing_ok=True)
   row,data=run(['--replace-body','--name',name,'--from',str(FRAG),'--plan',str(plan),str(copy)])
   emit(dict(kind='file',case='save_plan',size=size,state='first' if i==0 else 'repeat',sample=i,**row))
   row,data=run(['--apply',str(plan)]);assert copy.read_bytes()!=original
   emit(dict(kind='file',case='apply',size=size,state='first' if i==0 else 'repeat',sample=i,**row))
elif sys.argv[1]=='refresh':
 original=(CC/SMALL).read_bytes()
 try:
  for round in range(2):
   with service() as (env,pid,status):
    for i in range(4):
     if i:(CC/SMALL).write_bytes(original+f'\nexport const reportRefresh{i}=1;\n'.encode())
     row,data=run(CASES['outline'],env,pid,True);assert status()['generation']==i+1
     if i:assert f'reportRefresh{i}'.encode() in data
     emit(dict(kind='refresh',round=round,state='cold' if not i else 'edit',sample=i,**row))
     row,data=run(CASES['outline'],env,pid,True);assert status()['generation']==i+1
     emit(dict(kind='refresh',round=round,state='warm',sample=i,**row))
   (CC/SMALL).write_bytes(original)
 finally:(CC/SMALL).write_bytes(original)
elif sys.argv[1]=='direct':
 for i in range(3):
  row,data=run(CASES['outline']+['--direct']);check('outline',data);emit(dict(kind='direct',case='outline',sample=i,**row))
