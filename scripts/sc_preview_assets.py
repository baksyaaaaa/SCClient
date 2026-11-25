"""Usage: python3 scripts/sc_preview_assets.py packages/scclient_app/assets/sc_cosmetics [path to SC-Mod assets/scmod]
(needs Pillow). Run it again after changing hats, wings or capes in SC-Mod."""
"""Builds the launcher's wardrobe preview files from the SC mod's own cosmetics (assets/sc_cosmetics/):
hats and wings -> Bedrock .geo.json (one flat cube per face of the mod's block models, coloured from a palette PNG),
capes -> a normal 64x32-layout cape texture stitched from the mod's 4-strip cloth layout."""
import struct, json, math, os, sys
from PIL import Image

MOD = sys.argv[2] if len(sys.argv) > 2 else '../SC-Mod/src/main/resources/assets/scmod'
OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)
BRIGHT = 1.12   # the preview shades faces itself; the mod's colours already have shading baked in

def load(name):
    b = open(f'{MOD}/meshes/{name}.bin', 'rb').read(); o = 0
    nv = struct.unpack_from('>i', b, o)[0]; o += 4; P = []; N = []; C = []
    for _ in range(nv):
        x, y, z = struct.unpack_from('>fff', b, o); o += 12
        n = struct.unpack_from('>bbb', b, o); c = tuple(b[o + 3:o + 6]); o += 6
        P.append((x, y, z)); N.append(n); C.append(c)
    nq = struct.unpack_from('>i', b, o)[0]; o += 4
    Q = [struct.unpack_from('>hhhh', b, o + 8 * i) for i in range(nq)]
    return P, N, C, Q

class Palette:
    def __init__(self): self.idx = {}
    def get(self, c):
        c = tuple(min(255, round(v * BRIGHT)) for v in c)
        if c not in self.idx: self.idx[c] = len(self.idx)
        i = self.idx[c]; return (i % 32, i // 32)
    def save(self, path):
        rows = max(1, math.ceil(len(self.idx) / 32))
        im = Image.new('RGBA', (32, rows), (0, 0, 0, 0))
        for c, i in self.idx.items(): im.putpixel((i % 32, i // 32), c + (255,))
        im.save(path); return [32, rows]

# Java model space (pixels, y down, head/body pivot at the neck) -> Bedrock player space (y up, neck at y=24)
def to_bedrock(p): return (p[0], 24.0 - p[1], p[2])

def faces(P, N, C, Q, pal, offset=(0, 0, 0)):
    cubes = []
    for q in Q:
        pts = [to_bedrock(tuple(P[i][k] + offset[k] for k in range(3))) for i in q]
        # which way the face looks: from its corner order (the stored normals are flattened for the game's lighting)
        a0, a1, a2 = (P[q[0]], P[q[1]], P[q[2]])
        e1 = [a1[k] - a0[k] for k in range(3)]; e2 = [a2[k] - a0[k] for k in range(3)]
        n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]]
        axis = max(range(3), key=lambda a: abs(n[a]))
        # Java normal -> Bedrock normal (y flips)
        sign = (1 if n[axis] > 0 else -1) * (-1 if axis == 1 else 1)
        lo = [min(p[a] for p in pts) for a in range(3)]; hi = [max(p[a] for p in pts) for a in range(3)]
        size = [hi[a] - lo[a] for a in range(3)]; size[axis] = 0
        name = {(0, 1): 'east', (0, -1): 'west', (1, 1): 'up', (1, -1): 'down', (2, 1): 'south', (2, -1): 'north'}[(axis, sign)]
        u, v = pal.get(C[q[0]])
        cubes.append({"origin": [round(x, 4) for x in lo], "size": [round(x, 4) for x in size],
                      "uv": {name: {"uv": [u, v], "uv_size": [1, 1]}}})
    return cubes

def geo(ident, bones, tex):
    return {"format_version": "1.12.0", "minecraft:geometry": [{"description": {"identifier": ident, "texture_width": tex[0], "texture_height": tex[1]}, "bones": bones}]}

for h in range(1, 5):
    pal = Palette(); P, N, C, Q = load(f'hat_{h}')
    bones = [{"name": "sc_hat", "pivot": [0, 24, 0], "cubes": faces(P, N, C, Q, pal)}]
    tex = pal.save(f'{OUT}/hat_{h}.png')
    json.dump(geo(f"geometry.sc_hat_{h}", bones, tex), open(f'{OUT}/hat_{h}.geo.json', 'w'), separators=(',', ':'))

FLAP = 0.45   # radians: the wings' resting spread in game (0.38 + 0.2 * sin)
for w in range(1, 6):
    pal = Palette(); bones = []
    for side, suffix in ((-1, 'R'), (1, 'L')):
        P, N, C, Q = load(f'wing_{w}_{suffix}')
        hinge = (side * 1.5, 3.2 + 2.5, 2.6)   # body space: 0.2 blocks down, then the shoulder hinge
        bones.append({"name": f"sc_wing_{suffix}", "pivot": list(to_bedrock(hinge)),
                      "rotation": [0, round(math.degrees(-side * FLAP) * float(os.environ.get('WING_SIGN', '1')), 3), 0],
                      "cubes": faces(P, N, C, Q, pal, hinge)})
    tex = pal.save(f'{OUT}/wings_{w}.png')
    json.dump(geo(f"geometry.sc_wings_{w}", bones, tex), open(f'{OUT}/wings_{w}.geo.json', 'w'), separators=(',', ':'))

# capes: the mod's texture is a 10x4x1 box per strip (4 strips, 5 rows apart); stitch them into one 10x16x1 box
S = 12
for c in range(1, 29):
    src = Image.open(f'{MOD}/textures/entity/cape_{c}.png').convert('RGBA')
    out = Image.new('RGBA', (64 * S, 32 * S), (0, 0, 0, 0))
    def put(sx, sy, w, h, dx, dy): out.paste(src.crop((sx * S, sy * S, (sx + w) * S, (sy + h) * S)), (dx * S, dy * S))
    for s in range(4):
        y = 5 * s + 1
        put(0, y, 1, 4, 0, 1 + 4 * s)     # west side
        put(1, y, 10, 4, 1, 1 + 4 * s)    # north
        put(11, y, 1, 4, 11, 1 + 4 * s)   # east side
        put(12, y, 10, 4, 12, 1 + 4 * s)  # south
    put(1, 0, 10, 1, 1, 0)                # top of the first strip
    put(11, 15, 10, 1, 11, 0)             # bottom of the last strip
    out = out.resize((64 * 6, 32 * 6), Image.NEAREST)
    out.save(f'{OUT}/cape_{c}.png', optimize=True)
print('ok', sum(os.path.getsize(os.path.join(OUT, f)) for f in os.listdir(OUT)) // 1024, 'KB')
