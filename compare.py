import socket, struct, json, os, time, sys
import subprocess

# use as : $ python3 compare.py <file_name>
QUERY = sys.argv[1] if len(sys.argv) > 1 else 'spass'

print(f"Benchmarking search query: '{QUERY}'\n")

# Search (Spotlight/mdfind)
# This queries the system-wide Spotlight index directly
start_mac = time.time()
try:
    result = subprocess.run(['mdfind', QUERY], capture_output=True, text=True)
    mac_lines = [line for line in (result.stdout.strip().split('\n') if result.stdout else []) if line]
    mac_results_count = len(mac_lines)
except Exception as e:
    mac_lines = []
    mac_results_count = 0
    print(f"mdfind error: {e}")
mac_duration_ms = (time.time() - start_mac) * 1000

#  Qfinder 
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
try:
    s.connect(os.path.expanduser('~/.qfinder/socket.sock'))
    req = json.dumps({'command':'search','query':QUERY}).encode()
    
    start_qfinder = time.time()
    s.sendall(struct.pack('>I', len(req)) + req)
    
    n = struct.unpack('>I', s.recv(4))[0]
    d = b''
    while len(d) < n: 
        d += s.recv(n - len(d))
        
    qfinder_duration_ms = (time.time() - start_qfinder) * 1000
    qfinder_results = json.loads(d)
finally:
    s.close()

# ughhhh formatting
def get_path(r):
    if isinstance(r, dict):
        return r.get('path', str(r))
    return str(r)

top_qfinder = [get_path(r) for r in qfinder_results[:3]]
top_mac = mac_lines[:3]

# Pad lists to have 3 rows
top_qfinder += [""] * (3 - len(top_qfinder))
top_mac += [""] * (3 - len(top_mac))

def truncate(s, length=55):
    if not s: return "".ljust(length)
    if len(s) > length:
        return "..." + s[-(length-3):]
    return s.ljust(length)

header_qfinder = f"⚡️ Qfinder ({qfinder_duration_ms:.2f} ms, {len(qfinder_results)} res)"
header_mac = f" macOS Native ({mac_duration_ms:.2f} ms, {mac_results_count} res)"

print(f"| {header_qfinder.ljust(55)} | {header_mac.ljust(55)} |")
print(f"| {'-' * 55} | {'-' * 55} |")

for i in range(3):
    q_val = truncate(top_qfinder[i], 55)
    m_val = truncate(top_mac[i], 55)
    print(f"| {q_val} | {m_val} |")
    
print()