import {
  type LatLng,
  approxMeters,
  decodePolyline,
  projectPointOnLineRange,
  projectStopToLine,
} from "./polylineUtils";
import { patchSurabayaCorridorPoints } from "./sbPolylinePatches";
import {
  type Bus,
  type BusStop,
  type Corridor,
  type Direction,
  type StopArrival,
  type StopRouteBadge,
  busMatchesDirection,
  compareCorridorCodes,
  corridorCode,
  corridorHasTwoDirections,
  extraStopRoutes,
  findCorridorByCode,
  formatStopLabel,
  lookupCorridorValue,
  normalizeColor,
  resolveCorridorAlias,
  sameCorridorCode,
  trackingCorridorKey,
  wordOverlap,
} from "./transitHelpers";
import { patchYogyakartaCorridorPoints } from "./ykPolylinePatches";
import { getCustomCorridorPoints } from "./customRoutes";

const legPointsCache = new WeakMap<Corridor, { a?: LatLng[]; b?: LatLng[] }>();

function patchCorridorPoints(
  code: string,
  dir: Direction,
  pts: LatLng[],
  pref = "3501",
): LatLng[] {
  if (pts.length < 2) return pts;
  if (pref === "1") return patchYogyakartaCorridorPoints(code, dir, pts);
  if (pref === "3501" || !pref) return patchSurabayaCorridorPoints(code, dir, pts);
  return pts;
}

export function corridorLegPoints(corridor: Corridor, dir: Direction): LatLng[] {
  let entry = legPointsCache.get(corridor);
  if (!entry) {
    entry = {};
    legPointsCache.set(corridor, entry);
  }
  const hit = entry[dir];
  if (hit) return hit;
  const code = corridorCode(corridor);
  const pref = String(corridor.pref ?? "3501");
  const custom = getCustomCorridorPoints(pref, code, dir);
  if (custom && custom.length >= 2) {
    entry[dir] = custom;
    return custom;
  }
  const encoded = dir === "a" ? corridor.points_a : corridor.points_b;
  const pts = patchCorridorPoints(code, dir, decodePolyline(encoded), pref);
  entry[dir] = pts;
  return pts;
}

export function stopServingCorridors(
  stop: BusStop,
  allStops: BusStop[],
  colorMap: Record<string, string>,
  pref = "3501",
): StopRouteBadge[] {
  const isSurabaya = pref === "3501";
  const coordKey = `${stop.lat.toFixed(6)},${stop.lng.toFixed(6)}`;
  const candidates: BusStop[] = [stop];
  for (const s of allStops) {
    if (s === stop) continue;
    if (s.sh_id === stop.sh_id || `${s.lat.toFixed(6)},${s.lng.toFixed(6)}` === coordKey) {
      candidates.push(s);
    }
  }

  const map = new Map<string, { color: string; trackable: boolean }>();
  for (const entry of candidates) {
    for (let i = 0; i < entry.corridors.length; i++) {
      const code = entry.corridors[i].trim().toUpperCase();
      if (!code || (isSurabaya && code === "SBT")) continue;
      if (!map.has(code)) {
        const color =
          lookupCorridorValue(colorMap, code) ??
          entry.corridorColors[i] ??
          "#64748b";
        map.set(code, { color, trackable: true });
      }
    }
  }

  for (const entry of candidates) {
    for (const extra of extraStopRoutes(entry, pref)) {
      if (!map.has(extra.code)) {
        map.set(extra.code, { color: extra.color, trackable: false });
      }
    }
  }

  return Array.from(map.entries())
    .sort((a, b) => compareCorridorCodes(a[0], b[0]))
    .map(([code, item]) => ({
      code,
      color: item.color,
      trackable: item.trackable,
    }));
}

const stopPositionsCache = new WeakMap<BusStop[], WeakMap<LatLng[], number[]>>();

function buildStopPositions(legStops: BusStop[], line: LatLng[]): number[] {
  let byLine = stopPositionsCache.get(legStops);
  if (!byLine) {
    byLine = new WeakMap();
    stopPositionsCache.set(legStops, byLine);
  }
  const hit = byLine.get(line);
  if (hit) return hit;

  const positions: number[] = [];
  let cursor = 0;
  for (const s of legStops) {
    if (s.sh_id.startsWith("custom-")) {
      positions.push(projectStopToLine(s.lat, s.lng, line).pos);
      continue;
    }
    let proj = projectPointOnLineRange(
      s.lat,
      s.lng,
      line,
      Math.max(0, cursor - 80),
      Infinity,
    );
    if (proj.dist > 140) {
      proj = projectStopToLine(s.lat, s.lng, line);
    }
    positions.push(proj.pos);
    cursor = Math.max(cursor, proj.pos);
  }
  byLine.set(line, positions);
  return positions;
}

function resolveStopDirectionForCorridor(
  stop: BusStop,
  corridor: Corridor,
): Direction | null {
  const tw = stop.toward.trim().toLowerCase();
  if (!tw) return null;
  if (tw === "a" || tw === "a>b" || tw === "a > b") return "a";
  if (tw === "b" || tw === "b>a" || tw === "b > a") return "b";
  const corrToward = (corridor.toward ?? "").trim().toLowerCase();
  const corrOrigin = (corridor.origin ?? "").trim().toLowerCase();
  if (corrToward && corrOrigin && corrToward === corrOrigin) return null;
  if (corrToward && tw === corrToward) return "a";
  if (corrOrigin && tw === corrOrigin) return "b";
  const scoreA = corrToward ? wordOverlap(tw, corrToward) : 0;
  const scoreB = corrOrigin ? wordOverlap(tw, corrOrigin) : 0;
  if (scoreA > scoreB && scoreA > 0) return "a";
  if (scoreB > scoreA && scoreB > 0) return "b";
  return null;
}

const MAX_ARRIVAL_DISTANCE_M = 5000;
const MAX_ARRIVAL_ETA_S = 20 * 60;

export function computeStopArrivals(
  stop: BusStop,
  corridors: Corridor[],
  allStops: BusStop[],
  legStopsMap: Record<string, BusStop[]>,
  buses: Record<string, Bus>,
  colorMap: Record<string, string>,
  pref = "3501",
): StopArrival[] {
  const stopNameMap = new Map<string, string>();
  for (const s of allStops) {
    stopNameMap.set(s.sh_id, s.sh_name);
  }
  for (const list of Object.values(legStopsMap)) {
    for (const s of list) {
      stopNameMap.set(s.sh_id, s.sh_name);
    }
  }

  const badges = stopServingCorridors(stop, allStops, colorMap, pref).filter(
    (b) => b.trackable,
  );
  const bestByBus = new Map<string, StopArrival>();

  for (const badge of badges) {
    const code = badge.code;
    const corridor = findCorridorByCode(corridors, code);
    if (!corridor) continue;
    const corridorColor = normalizeColor(corridor.color) ?? colorMap[code] ?? badge.color;
    const forcedDir = resolveStopDirectionForCorridor(stop, corridor);
    const candidateDirs: Direction[] = !corridorHasTwoDirections(corridor)
      ? ["a"]
      : forcedDir
        ? [forcedDir]
        : ["a", "b"];
    const exactDirs = candidateDirs.filter((d) =>
      legStopsMap[`${code}|${d}`]?.some((s) => s.sh_id === stop.sh_id),
    );
    const activeDirs = exactDirs.length > 0 ? exactDirs : candidateDirs;

    for (const dir of activeDirs) {
      const legKey = `${code}|${dir}`;
      const leg = legStopsMap[legKey];
      const line = corridorLegPoints(corridor, dir);
      if (line.length < 2) continue;

      const stopPositions = leg && leg.length > 0 ? buildStopPositions(leg, line) : [];
      let targetIdx = leg ? leg.findIndex((s) => s.sh_id === stop.sh_id) : -1;
      let isCustomTarget = stop.sh_id.startsWith("custom-");
      let targetPos = targetIdx >= 0 ? stopPositions[targetIdx] : 0;

      if (targetIdx < 0) {
        if (exactDirs.length > 0) continue;
        if (leg && leg.length > 0 && !forcedDir) {
          let bestStopDist = 25;
          for (let i = 0; i < leg.length; i++) {
            const d = approxMeters([leg[i].lat, leg[i].lng], [stop.lat, stop.lng]);
            if (d <= bestStopDist) {
              bestStopDist = d;
              targetIdx = i;
            }
          }
          if (targetIdx >= 0) {
            targetPos = stopPositions[targetIdx];
          }
        }
        if (targetIdx < 0) {
          if (forcedDir !== dir && !isCustomTarget) continue;
          const directProj = projectStopToLine(stop.lat, stop.lng, line);
          if (directProj.dist > 120) continue;
          targetPos = directProj.pos;
          isCustomTarget = true;
        }
      }

      for (const bus of Object.values(buses)) {
        if (bus.stale || !bus.corridor) continue;
        const busCode = resolveCorridorAlias(
          corridors,
          trackingCorridorKey(bus.corridor.trim().toUpperCase(), pref),
        );
        if (!sameCorridorCode(busCode, code)) continue;
        if (!busMatchesDirection(bus, corridor, dir)) continue;

        const nextIdx =
          bus.next_halte && leg ? leg.findIndex((s) => s.sh_id === bus.next_halte) : -1;
        const prevIdx =
          bus.prev_halte && leg ? leg.findIndex((s) => s.sh_id === bus.prev_halte) : -1;

        let busProj: { dist: number; pos: number };
        if (nextIdx >= 0) {
          if (isCustomTarget) {
            if (stopPositions[nextIdx] > targetPos + 120) continue;
          } else {
            if (nextIdx > targetIdx) continue;
            if (targetIdx === 0 && nextIdx > 0) continue;
          }
          const anchorPrevPos =
            prevIdx >= 0 && prevIdx <= nextIdx
              ? stopPositions[prevIdx]
              : nextIdx > 0
                ? stopPositions[nextIdx - 1]
                : 0;
          const minBound = Math.max(0, anchorPrevPos - 180);
          const maxBound = stopPositions[nextIdx] + 180;
          busProj = projectPointOnLineRange(
            bus.lat,
            bus.lng,
            line,
            minBound,
            maxBound,
          );
        } else {
          busProj = projectStopToLine(bus.lat, bus.lng, line);
          if (busProj.dist > 450) continue;
          if (!isCustomTarget && targetIdx === 0) {
            if (
              busProj.pos > targetPos + 40 ||
              approxMeters([bus.lat, bus.lng], [stop.lat, stop.lng]) > 150
            ) {
              continue;
            }
          }
          if (busProj.pos > targetPos + 50) continue;
        }

        let remainingMeters: number;
        if (!isCustomTarget && nextIdx === targetIdx && busProj.pos > targetPos) {
          const direct = approxMeters([bus.lat, bus.lng], [stop.lat, stop.lng]);
          if (direct > 160) continue;
          remainingMeters = direct;
        } else if (busProj.pos > targetPos + 50) {
          continue;
        } else {
          remainingMeters = Math.max(0, targetPos - busProj.pos);
        }
        if (remainingMeters > MAX_ARRIVAL_DISTANCE_M) continue;

        let inferredNextIdx: number;
        if (nextIdx >= 0) {
          inferredNextIdx = nextIdx;
        } else {
          const foundIdx = stopPositions.findIndex((p) => p >= busProj.pos - 20);
          inferredNextIdx = foundIdx >= 0 ? foundIdx : Math.max(0, targetIdx);
        }
        const stopsBetween = isCustomTarget
          ? stopPositions.filter((p) => p > busProj.pos + 20 && p < targetPos - 20).length
          : Math.max(0, targetIdx - Math.max(0, inferredNextIdx));

        const baseKmh = 20;
        const rawSpeed = bus.speed ?? 0;
        const effectiveKmh =
          rawSpeed >= 8
            ? 0.45 * Math.min(42, Math.max(8, rawSpeed)) + 0.55 * baseKmh
            : remainingMeters < 250
              ? 14
              : 18;
        const etaSeconds = Math.max(
          0,
          Math.round(remainingMeters / (effectiveKmh / 3.6) + stopsBetween * 18),
        );
        if (etaSeconds > MAX_ARRIVAL_ETA_S) continue;
        const isArrivingNow = etaSeconds < 60 || remainingMeters <= 120;
        const etaMinutes = isArrivingNow ? 1 : Math.max(1, Math.round(etaSeconds / 60));

        const prevName = bus.prev_halte ? stopNameMap.get(bus.prev_halte) : undefined;
        const nextName = bus.next_halte ? stopNameMap.get(bus.next_halte) : undefined;
        let segmentText = "";
        if (prevName && nextName && prevName !== nextName) {
          segmentText = `${prevName} → ${nextName}`;
        } else if (nextName) {
          segmentText = `Menuju ${nextName}`;
        } else if (leg && inferredNextIdx > 0 && leg[inferredNextIdx - 1] && leg[inferredNextIdx]) {
          segmentText = `${leg[inferredNextIdx - 1].sh_name} → ${leg[inferredNextIdx].sh_name}`;
        } else if (leg && leg[inferredNextIdx]) {
          segmentText = `Menuju ${leg[inferredNextIdx].sh_name}`;
        }

        const destination = formatStopLabel(
          bus.toward?.trim() ||
            (dir === "a" ? corridor.toward?.trim() : corridor.origin?.trim()) ||
            "",
        );
        const fixAtMs = bus.receivedAt - bus.fix_age_s * 1000;
        const arrival: StopArrival = {
          busId: bus.id,
          busName: bus.name?.trim() || bus.plate?.trim() || bus.id,
          plate: bus.plate,
          corridor: code,
          corridorColor,
          etaSeconds,
          etaMinutes,
          isArrivingNow,
          distanceMeters: Math.round(remainingMeters),
          segmentText,
          destination,
          fixAtMs,
          speedKmh:
            bus.speed != null && Number.isFinite(bus.speed)
              ? Math.round(bus.speed)
              : null,
        };

        const existing = bestByBus.get(bus.id);
        if (!existing || arrival.etaSeconds < existing.etaSeconds) {
          bestByBus.set(bus.id, arrival);
        }
      }
    }
  }

  return Array.from(bestByBus.values()).sort(
    (a, b) => a.etaSeconds - b.etaSeconds || a.distanceMeters - b.distanceMeters,
  );
}
