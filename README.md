#### Qfinder
File search like trivial task is bloated and slow even when used by every mac owners on a daily basis. <br>
This tries to fix that file search keeping it sub 10 milli seconds.
Compare it yourself try running it on your Mac .

##### Installation

```bash
curl -fsSL https://raw.githubusercontent.com/sp4s-s/Qfinder/main/install.sh | bash
```
<br>
**in-case-doesnt start/load**
clone it 
move to /extension and run it once
```
git clone https://github.com/sp4s-s/Qfinder.git
cd extension
bun dev 
```

#### Compare it by running
```python
python3 compare.py file
# broken name search
python3 compare.py "spas xyz.pdf"
```

#### Results
(modal-env) quicktip > python3 compare.py "spas xyz.pdf"
Benchmarking search query: 'spas xyz.pdf'

| ⚡️ Qfinder (4.32 ms, 1 res)                             |  macOS Native (82.48 ms, 0 res)                        |
| ------------------------------------------------------- | ------------------------------------------------------- |
| /Users/sqs/Desktop/cvs/spass_m4 _sr_xyz.pdf           |                                                         |
|                                                         |                                                         |
|                                                         |                                                         |
