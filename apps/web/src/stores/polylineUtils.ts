export type LatLng = [number, number];

export function encodePolyline(pts: LatLng[]): string {
  let out = "";
  let prevLat = 0;
  let prevLng = 0;
  const writeSigned = (v: number) => {
    let val = v < 0 ? ~(v << 1) : v << 1;
    while (val >= 0x20) {
      out += String.fromCharCode((0x20 | (val & 0x1f)) + 63);
      val >>= 5;
    }
    out += String.fromCharCode(val + 63);
  };
  for (const [lat, lng] of pts) {
    const ilat = Math.round(lat * 1e5);
    const ilng = Math.round(lng * 1e5);
    writeSigned(ilat - prevLat);
    writeSigned(ilng - prevLng);
    prevLat = ilat;
    prevLng = ilng;
  }
  return out;
}

export function decodePolyline(encoded?: string | null): LatLng[] {
  if (!encoded) return [];
  const pts: LatLng[] = [];
  let i = 0;
  let lat = 0;
  let lng = 0;
  while (i < encoded.length) {
    let shift = 0;
    let res = 0;
    let b = 0;
    do {
      if (i >= encoded.length) return pts;
      b = encoded.charCodeAt(i++) - 63;
      res |= (b & 0x1f) << shift;
      shift += 5;
    } while (b >= 0x20);
    lat += res & 1 ? ~(res >> 1) : res >> 1;

    shift = 0;
    res = 0;
    do {
      if (i >= encoded.length) return pts;
      b = encoded.charCodeAt(i++) - 63;
      res |= (b & 0x1f) << shift;
      shift += 5;
    } while (b >= 0x20);
    lng += res & 1 ? ~(res >> 1) : res >> 1;

    pts.push([lat / 1e5, lng / 1e5]);
  }
  return pts;
}

export function approxMeters(a: LatLng, b: LatLng): number {
  const cosLat = Math.cos((((a[0] + b[0]) / 2) * Math.PI) / 180);
  return Math.hypot((a[1] - b[1]) * cosLat, a[0] - b[0]) * 111320;
}

export function findClosestInRange(
  pts: LatLng[],
  target: LatLng,
  from: number,
  to: number,
  maxMeters: number,
): number {
  let bestIdx = -1;
  let bestDist = maxMeters;
  const start = Math.max(0, from);
  const end = Math.min(pts.length - 1, to);
  for (let i = start; i <= end; i++) {
    const d = approxMeters(pts[i], target);
    if (d <= bestDist) {
      bestDist = d;
      bestIdx = i;
    }
  }
  return bestIdx;
}

export function stripSpur(
  pts: LatLng[],
  tip: LatLng,
  road: LatLng,
  tipMax = 8,
  roadMax = 20,
  span = 8,
): LatLng[] {
  const tipIdx = findClosestInRange(pts, tip, 0, pts.length - 1, tipMax);
  if (tipIdx < 0) return pts;
  const enterIdx = findClosestInRange(pts, road, tipIdx - span, tipIdx - 1, roadMax);
  const exitIdx = findClosestInRange(pts, road, tipIdx + 1, tipIdx + span, roadMax);
  if (enterIdx < 0 || exitIdx <= enterIdx) return pts;
  const nextIdx = approxMeters(pts[enterIdx], pts[exitIdx]) <= 2 ? exitIdx + 1 : exitIdx;
  return [...pts.slice(0, enterIdx + 1), ...pts.slice(nextIdx)];
}

export function replaceDetour(
  pts: LatLng[],
  detour: LatLng,
  enter: LatLng,
  exit: LatLng,
  replacement: LatLng[] = [],
  opts: {
    detourMax?: number;
    enterMax?: number;
    exitMax?: number;
    beforeSpan?: number;
    afterSpan?: number;
  } = {},
): LatLng[] {
  const {
    detourMax = 15,
    enterMax = 15,
    exitMax = 15,
    beforeSpan = 10,
    afterSpan = 10,
  } = opts;
  const detourIdx = findClosestInRange(pts, detour, 0, pts.length - 1, detourMax);
  if (detourIdx < 0) return pts;
  const enterIdx = findClosestInRange(pts, enter, detourIdx - beforeSpan, detourIdx - 1, enterMax);
  const exitIdx = findClosestInRange(pts, exit, detourIdx + 1, detourIdx + afterSpan, exitMax);
  if (enterIdx < 0 || exitIdx <= enterIdx) return pts;
  return [...pts.slice(0, enterIdx + 1), ...replacement, ...pts.slice(exitIdx)];
}

export function replaceBetween(
  pts: LatLng[],
  enter: LatLng,
  exit: LatLng,
  replacement: LatLng[] = [],
  opts: {
    enterMax?: number;
    exitMax?: number;
    maxSpan?: number;
  } = {},
): LatLng[] {
  const { enterMax = 15, exitMax = 15, maxSpan = 25 } = opts;
  let i = 0;
  while (i < pts.length - 1) {
    if (approxMeters(pts[i], enter) <= enterMax) {
      let bestEnter = i;
      let bestEnterDist = approxMeters(pts[i], enter);
      let j = i + 1;
      while (j < pts.length - 1 && approxMeters(pts[j], enter) <= enterMax) {
        const d = approxMeters(pts[j], enter);
        if (d < bestEnterDist) {
          bestEnterDist = d;
          bestEnter = j;
        }
        j++;
      }
      const exitIdx = findClosestInRange(
        pts,
        exit,
        bestEnter + 1,
        bestEnter + maxSpan,
        exitMax,
      );
      if (exitIdx > bestEnter) {
        return [...pts.slice(0, bestEnter + 1), ...replacement, ...pts.slice(exitIdx)];
      }
      i = j;
    } else {
      i++;
    }
  }
  return pts;
}

export function projectStopToLine(
  lat: number,
  lng: number,
  line: LatLng[],
): { dist: number; pos: number } {
  let bestDist = Infinity;
  let bestPos = 0;
  let acc = 0;
  for (let i = 0; i < line.length - 1; i++) {
    const a = line[i];
    const b = line[i + 1];
    const cosLat = Math.cos((((a[0] + b[0]) / 2) * Math.PI) / 180);
    const ax = a[1] * cosLat;
    const ay = a[0];
    const bx = b[1] * cosLat;
    const by = b[0];
    const px = lng * cosLat;
    const py = lat;
    const vx = bx - ax;
    const vy = by - ay;
    const l2 = vx * vx + vy * vy;
    const t =
      l2 > 0 ? Math.max(0, Math.min(1, ((px - ax) * vx + (py - ay) * vy) / l2)) : 0;
    const qx = ax + t * vx;
    const qy = ay + t * vy;
    const dist = Math.hypot(px - qx, py - qy) * 111320;
    const segLen = Math.hypot(vx, vy) * 111320;
    if (dist < bestDist) {
      bestDist = dist;
      bestPos = acc + t * segLen;
    }
    acc += segLen;
  }
  return { dist: bestDist, pos: bestPos };
}

export function projectPointOnLineRange(
  lat: number,
  lng: number,
  line: LatLng[],
  minPos = -Infinity,
  maxPos = Infinity,
): { dist: number; pos: number } {
  let bestDist = Infinity;
  let bestPos = 0;
  let acc = 0;
  for (let i = 0; i < line.length - 1; i++) {
    const a = line[i];
    const b = line[i + 1];
    const cosLat = Math.cos((((a[0] + b[0]) / 2) * Math.PI) / 180);
    const ax = a[1] * cosLat;
    const ay = a[0];
    const bx = b[1] * cosLat;
    const by = b[0];
    const vx = bx - ax;
    const vy = by - ay;
    const segLen = Math.hypot(vx, vy) * 111320;
    const segEnd = acc + segLen;
    if (segEnd >= minPos && acc <= maxPos) {
      const px = lng * cosLat;
      const py = lat;
      const l2 = vx * vx + vy * vy;
      const t =
        l2 > 0 ? Math.max(0, Math.min(1, ((px - ax) * vx + (py - ay) * vy) / l2)) : 0;
      const qx = ax + t * vx;
      const qy = ay + t * vy;
      const dist = Math.hypot(px - qx, py - qy) * 111320;
      if (dist < bestDist) {
        bestDist = dist;
        bestPos = acc + t * segLen;
      }
    }
    acc = segEnd;
  }
  if (bestDist === Infinity) {
    return projectStopToLine(lat, lng, line);
  }
  return { dist: bestDist, pos: bestPos };
}
