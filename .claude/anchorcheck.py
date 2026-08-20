import re, collections, sys
p = sys.argv[1]
t = open(p).read()
anchors = set(re.findall(r'<a\s+name="([^"]+)"', t))
def slug(h):
    s = h.strip().lower()
    s = re.sub(r'[^\w\s-]', '', s)
    s = re.sub(r'\s+', '-', s)
    return s
heads = []
for line in t.split('\n'):
    m = re.match(r'^(#{2,6})\s+(.*)$', line)
    if m:
        heads.append(slug(m.group(2)))
anchors |= set(heads)
links = re.findall(r'\]\(#([^)]+)\)', t)
missing = collections.Counter(l for l in links if l not in anchors)
print("MISSING ANCHORS:")
for k, v in missing.items():
    print("  ", k, v)
print()
print("ALL LINK TARGETS USED:")
for k, v in collections.Counter(links).most_common():
    print("  ", k, v)
