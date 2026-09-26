import zlib, struct, sys

def read_png(path):
    d = open(path,'rb').read()
    assert d[:8] == b'\x89PNG\r\n\x1a\n'
    pos, idat, pal, trns = 8, b'', None, None
    w=h=bd=ct=0
    while pos < len(d):
        ln = struct.unpack('>I', d[pos:pos+4])[0]; typ = d[pos+4:pos+8]
        body = d[pos+8:pos+8+ln]
        if typ == b'IHDR':
            w,h,bd,ct,_,_,_ = struct.unpack('>IIBBBBB', body)
        elif typ == b'IDAT': idat += body
        elif typ == b'PLTE': pal = body
        elif typ == b'tRNS': trns = body
        pos += 12 + ln
    raw = zlib.decompress(idat)
    assert bd == 8, f"bit depth {bd}"
    nch = {0:1,2:3,3:1,4:2,6:4}[ct]
    stride = w*nch
    out, prev = [], bytearray(stride)
    p = 0
    for y in range(h):
        f = raw[p]; p += 1
        line = bytearray(raw[p:p+stride]); p += stride
        for i in range(stride):
            a = line[i-nch] if i >= nch else 0
            b = prev[i]
            c = prev[i-nch] if i >= nch else 0
            if f==1: line[i] = (line[i]+a)&255
            elif f==2: line[i] = (line[i]+b)&255
            elif f==3: line[i] = (line[i]+(a+b)//2)&255
            elif f==4:
                pp=a+b-c; pa=abs(pp-a); pb=abs(pp-b); pc=abs(pp-c)
                pr = a if (pa<=pb and pa<=pc) else (b if pb<=pc else c)
                line[i] = (line[i]+pr)&255
        out.append(bytes(line)); prev = line
    # normalise to RGBA
    px = []
    for line in out:
        row = []
        for x in range(w):
            v = line[x*nch:(x+1)*nch]
            if ct == 6: row.append(tuple(v))
            elif ct == 2: row.append((v[0],v[1],v[2],255))
            elif ct == 3:
                i = v[0]; r,g,b = pal[i*3:i*3+3]
                a = trns[i] if trns and i < len(trns) else 255
                row.append((r,g,b,a))
            elif ct == 0: row.append((v[0],)*3+(255,))
            elif ct == 4: row.append((v[0],)*3+(v[1],))
        px.append(row)
    return w,h,px

def write_indexed(path, w, h, px, palette):
    idx = {c:i for i,c in enumerate(palette)}
    raw = bytearray()
    for row in px:
        raw.append(0)
        for c in row: raw.append(idx[c])
    plte = b''.join(bytes(c[:3]) for c in palette)
    trns = bytes(c[3] for c in palette)
    def chunk(t, b):
        return struct.pack('>I', len(b)) + t + b + struct.pack('>I', zlib.crc32(t+b)&0xffffffff)
    out = b'\x89PNG\r\n\x1a\n'
    out += chunk(b'IHDR', struct.pack('>IIBBBBB', w,h,8,3,0,0,0))
    out += chunk(b'PLTE', plte)
    if any(a < 255 for a in trns): out += chunk(b'tRNS', trns)
    out += chunk(b'IDAT', zlib.compress(bytes(raw), 9))
    out += chunk(b'IEND', b'')
    open(path,'wb').write(out)

if __name__ == '__main__':
    w,h,px = read_png(sys.argv[1])
    cols = {}
    for row in px:
        for c in row: cols[c] = cols.get(c,0)+1
    print(f"{w}x{h}, {len(cols)} unique colours")
