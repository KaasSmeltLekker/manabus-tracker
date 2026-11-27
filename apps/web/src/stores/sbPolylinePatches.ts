import {
  type LatLng,
  approxMeters,
  findClosestInRange,
  replaceDetour,
  stripSpur,
} from "./polylineUtils";
import type { Direction } from "./transitHelpers";

const FD4_STASIUN_KOTA_JUNCTION: LatLng = [-7.24273, 112.74463];
const FD4_ATOM1_SPUR_TIP: LatLng = [-7.24103, 112.74470];
const FD4_ATOM_NORTH_GATE: LatLng = [-7.24064, 112.74514];
const FD4_SIAGA_LOOP: LatLng = [-7.24189, 112.74266];
const FD4_ATOM_SOUTH_EXIT: LatLng = [-7.24276, 112.74484];
const FD4_PASAR_ATOM_PATH: LatLng[] = [
  [-7.24064, 112.74508],
  [-7.24069, 112.74497],
  [-7.24079, 112.74484],
  [-7.24091, 112.74473],
  [-7.24103, 112.74470],
  [-7.24116, 112.74477],
  [-7.24124, 112.74481],
  [-7.24138, 112.74482],
  [-7.24147, 112.74486],
  [-7.24166, 112.74495],
  [-7.24171, 112.74497],
  [-7.24177, 112.74497],
  [-7.24205, 112.74479],
  [-7.24208, 112.74477],
  [-7.24212, 112.74477],
  [-7.24240, 112.74476],
  [-7.24247, 112.74474],
  [-7.24257, 112.74473],
  [-7.24262, 112.74475],
  [-7.24267, 112.74478],
  [-7.24272, 112.74482],
];
const FD10_BUNGURAN_NORTH_GATE: LatLng = [-7.24060, 112.74527];
const FD10_ATOM2_TERMINUS: LatLng = [-7.24275, 112.74485];
const FD10_UNIPRA_ROAD: LatLng = [-7.28965, 112.78399];
const FD10_UNIPRA_SPUR_TIP: LatLng = [-7.28971, 112.78399];
const FD10_ITATS2_ROAD: LatLng = [-7.28980, 112.77864];
const FD10_ITATS2_SPUR_TIP: LatLng = [-7.28994, 112.77900];
const FD10_RANGKAH_ROAD: LatLng = [-7.24669, 112.76903];
const FD10_RANGKAH_SPUR_TIP: LatLng = [-7.24669, 112.76908];
const FD10_SWK_ROAD: LatLng = [-7.25276, 112.76687];
const FD10_SWK_SPUR_TIP: LatLng = [-7.25280, 112.76698];
const FD10_PACAR_KELING_KINK: LatLng = [-7.26031, 112.75931];
const FD10_PACAR_KELING_ROAD: LatLng = [-7.26030, 112.75923];
const FD10_NAROTAMA_ROAD: LatLng = [-7.28960, 112.77633];
const FD10_NAROTAMA_SPUR_TIP: LatLng = [-7.28955, 112.77634];
const FD10_PUSKESMAS_ROAD: LatLng = [-7.28957, 112.77860];
const FD10_PUSKESMAS_SPUR_TIP: LatLng = [-7.28950, 112.77861];
const FD10_SMPN19_ROAD: LatLng = [-7.28866, 112.78062];
const FD10_SMPN19_SPUR_TIP: LatLng = [-7.28864, 112.78051];
const FD10_MARINA_ROAD_W: LatLng = [-7.29118, 112.80185];
const FD10_MARINA_DETOUR_NW: LatLng = [-7.29110, 112.80184];
const FD10_MARINA_ROAD_E: LatLng = [-7.29123, 112.80228];
const FD12_GUNUNG_ANYAR_MERR: LatLng = [-7.34590, 112.78627];
const FD12_GUNUNG_ANYAR_LOOP: LatLng = [-7.34685, 112.78612];
const R5_BENOWO_ROAD: LatLng = [-7.23485, 112.61464];
const R5_BENOWO_SPUR_TIP: LatLng = [-7.23471, 112.61469];
const R5_BIBIS_ROAD: LatLng = [-7.25821, 112.67617];
const R5_BIBIS_SPUR_TIP: LatLng = [-7.25811, 112.67619];
const R5_SIMO_NORTH: LatLng = [-7.26900, 112.71543];
const R5_SIMO_UTURN_TURN: LatLng = [-7.26916, 112.71566];
const FD7_BUBUTAN_ENTRY: LatLng = [-7.25534, 112.73414];
const FD7_KOBLEN_DETOUR: LatLng = [-7.25301, 112.73349];
const FD7_PRINGADI_REJOIN: LatLng = [-7.25281, 112.73453];
const FD7_BUBUTAN_NORTH: LatLng[] = [
  [-7.25523, 112.73414],
  [-7.25503, 112.73416],
  [-7.25480, 112.73415],
  [-7.25471, 112.73417],
  [-7.25457, 112.73425],
  [-7.25378, 112.73457],
  [-7.25341, 112.73470],
  [-7.25305, 112.73482],
  [-7.25291, 112.73486],
];
const FD7_GUBENG_ENTRY: LatLng = [-7.26809, 112.75101];
const FD7_GUBENG_DETOUR: LatLng = [-7.26998, 112.75161];
const FD7_GUBENG_REJOIN: LatLng = [-7.26789, 112.75178];
const FD7_BANDA_EAST: LatLng[] = [
  [-7.26808, 112.75106],
  [-7.26807, 112.75122],
  [-7.26799, 112.75149],
];
const FD7_MENUR_ROAD: LatLng = [-7.28682, 112.76329];
const FD7_MENUR_SPUR_TIP: LatLng = [-7.28696, 112.76351];
const FD7_NGINDEN_EAST: LatLng = [-7.29941, 112.76218];
const FD7_NGINDEN_WEST: LatLng = [-7.29938, 112.76200];
const FD7_NGINDEN_UTURN: LatLng[] = [
  [-7.29985, 112.76217],
  [-7.30006, 112.76217],
  [-7.30047, 112.76216],
  [-7.30047, 112.76200],
  [-7.30006, 112.76200],
  [-7.29984, 112.76200],
  [-7.29954, 112.76201],
];
const FD2_TEUKU_UMAR_TURN: LatLng = [-7.27929, 112.73776];
const FD2_KARTINI_SKIP: LatLng = [-7.27900, 112.73610];
const FD2_IMAM_BONJOL_REJOIN: LatLng = [-7.27881, 112.73501];
const FD2_WR_SUPRATMAN_PATH: LatLng[] = [
  [-7.27984, 112.73769],
  [-7.28036, 112.73762],
  [-7.28019, 112.73645],
  [-7.27999, 112.73536],
  [-7.27995, 112.73512],
  [-7.27969, 112.73512],
  [-7.27940, 112.73518],
  [-7.27908, 112.73525],
  [-7.27885, 112.73529],
];
const FD3_SEMOLOWARU_CORNER: LatLng = [-7.30081, 112.77923];
const FD3_SEMOLOWARU_JOG: LatLng = [-7.30082, 112.77930];
const FD3_SEMOLOWARU_SOUTH: LatLng = [-7.30087, 112.77922];
const FD7_PEMUDA_LOOP_START: LatLng = [-7.26594, 112.74963];
const FD7_PEMUDA_LOOP_SW: LatLng = [-7.26576, 112.74885];
const FD7_PEMUDA_LOOP_E: LatLng = [-7.26584, 112.74936];
const FD7_PEMUDA_SPUR_ROAD: LatLng = [-7.26522, 112.74761];
const FD7_PEMUDA_SPUR_TIP: LatLng = [-7.26528, 112.74759];
const YOS_SUDARSO_S: LatLng = [-7.26348, 112.74593];
const YOS_SUDARSO_LOOP_W: LatLng = [-7.26337, 112.74569];
const YOS_SUDARSO_N: LatLng = [-7.26338, 112.74595];
const FD8_JONO_NE_TURN: LatLng = [-7.28358, 112.68236];
const FD8_JONO_SW_UTURN: LatLng = [-7.28511, 112.68089];
const FD8_JONO_REJOIN: LatLng = [-7.28336, 112.68259];
const FD9_MENANGGAL_S: LatLng = [-7.34361, 112.72899];
const FD9_MENANGGAL_DIP: LatLng = [-7.34358, 112.72876];
const FD9_MENANGGAL_N: LatLng = [-7.34311, 112.72899];
const FD9_MENANGGAL_SPUR_ROAD: LatLng = [-7.34069, 112.72900];
const FD9_MENANGGAL_SPUR_TIP: LatLng = [-7.34069, 112.72888];
const FD9_PAGESANGAN_ROAD: LatLng = [-7.33285, 112.71088];
const FD9_PAGESANGAN_SPUR_TIP: LatLng = [-7.33292, 112.71087];
const FD9_DARMO_PARK_E: LatLng = [-7.29170, 112.71813];
const FD9_DARMO_PARK_DIP: LatLng = [-7.29188, 112.71811];
const FD9_DARMO_PARK_W: LatLng = [-7.29154, 112.71713];
const FD9_KARANGPOH_ROAD: LatLng = [-7.25899, 112.68305];
const FD9_KARANGPOH_SPUR_TIP: LatLng = [-7.25905, 112.68305];
const FD9_SATELIT_A_TURN: LatLng = [-7.26495, 112.69378];
const FD9_TANJUNGSARI_A_DETOUR: LatLng = [-7.26267, 112.69500];
const FD9_TANDES_A_REJOIN: LatLng = [-7.25926, 112.68496];
const FD9_DARMO_SATELIT_A: LatLng[] = [
  [-7.26482, 112.69372],
  [-7.26479, 112.69352],
  [-7.26464, 112.69197],
  [-7.26447, 112.69007],
  [-7.26446, 112.68995],
  [-7.26427, 112.68778],
  [-7.26425, 112.68762],
  [-7.26419, 112.68749],
  [-7.26406, 112.68616],
  [-7.26384, 112.68380],
  [-7.26337, 112.68385],
  [-7.26265, 112.68400],
  [-7.26189, 112.68420],
  [-7.26176, 112.68419],
  [-7.26154, 112.68428],
  [-7.26070, 112.68460],
  [-7.26024, 112.68477],
  [-7.25985, 112.68493],
  [-7.25963, 112.68498],
];
const FD9_TANDES_B_TURN: LatLng = [-7.25914, 112.68495];
const FD9_TANJUNGSARI_B_DETOUR: LatLng = [-7.26261, 112.69512];
const FD9_SATELIT_B_REJOIN: LatLng = [-7.26479, 112.69381];
const FD9_DARMO_SATELIT_B: LatLng[] = [
  [-7.25926, 112.68496],
  [-7.25963, 112.68498],
  [-7.25985, 112.68493],
  [-7.26024, 112.68477],
  [-7.26070, 112.68460],
  [-7.26154, 112.68428],
  [-7.26176, 112.68419],
  [-7.26189, 112.68420],
  [-7.26265, 112.68400],
  [-7.26337, 112.68385],
  [-7.26384, 112.68380],
  [-7.26406, 112.68616],
  [-7.26419, 112.68749],
  [-7.26416, 112.68765],
  [-7.26417, 112.68779],
  [-7.26437, 112.68996],
  [-7.26438, 112.69008],
  [-7.26455, 112.69198],
  [-7.26471, 112.69360],
  [-7.26476, 112.69378],
];
const FD9_ADITYA_ENTER: LatLng = [-7.29535, 112.73314];
const FD9_ADITYA_DETOUR: LatLng = [-7.29286, 112.72965];
const FD9_ADITYA_EXIT: LatLng = [-7.29538, 112.73323];
const FD9_CILIWUNG_B_ROAD: LatLng = [-7.29318, 112.73574];
const FD9_CILIWUNG_B_TIP: LatLng = [-7.29314, 112.73570];
const FD9_BALONG_N: LatLng = [-7.25872, 112.67858];
const FD9_BALONG_TIP: LatLng = [-7.25923, 112.67862];
const FD9_BALONG_S: LatLng = [-7.26396, 112.67745];
const FD9_BALONG_BAY_A: LatLng[] = [
  [-7.25898, 112.67852],
  [-7.25901, 112.67861],
  [-7.25904, 112.67863],
  [-7.25908, 112.67864],
  [-7.25923, 112.67862],
  [-7.25931, 112.67860],
  [-7.25932, 112.67857],
  [-7.25928, 112.67846],
];
const FD9_BALONG_BAY_B: LatLng[] = [
  ...FD9_BALONG_BAY_A,
  [-7.25898, 112.67852],
];
const FD7_GENTENG_1_ROAD: LatLng = [-7.25610, 112.73968];
const FD7_GENTENG_1_TIP: LatLng = [-7.25618, 112.73968];
const FD7_GENTENG_2_ROAD: LatLng = [-7.25607, 112.73845];
const FD7_GENTENG_2_TIP: LatLng = [-7.25613, 112.73845];
const FD7_SUMBAWA_ENTER: LatLng = [-7.27496, 112.74659];
const FD7_SUMBAWA_DETOUR: LatLng = [-7.27501, 112.74642];
const FD7_SUMBAWA_EXIT: LatLng = [-7.27447, 112.74677];
const FD3_WONOKROMO_ENTER: LatLng = [-7.30233, 112.73857];
const FD3_WONOKROMO_DETOUR: LatLng = [-7.30254, 112.73857];
const FD3_WONOKROMO_EXIT: LatLng = [-7.30287, 112.73814];
const FD3_WONOKROMO_MAIN: LatLng[] = [
  [-7.30257, 112.73838],
  [-7.30267, 112.73830],
];
const FD5_PTC_A_ENTER: LatLng = [-7.28744, 112.67838];
const FD5_PTC_A_DETOUR: LatLng = [-7.28718, 112.67848];
const FD5_PTC_A_EXIT: LatLng = [-7.28706, 112.67847];
const FD5_PTC_OUTER_CURVE: LatLng[] = [
  [-7.28742, 112.67844],
  [-7.28740, 112.67854],
  [-7.28736, 112.67861],
  [-7.28721, 112.67855],
  [-7.28714, 112.67853],
];
const FD5_PTC_B_ENTER: LatLng = [-7.28730, 112.67831];
const FD5_PTC_B_DETOUR: LatLng = [-7.28737, 112.67850];
const FD5_PTC_B_EXIT: LatLng = [-7.28685, 112.67828];
const FD5_PTC_B_OUTER_CURVE: LatLng[] = [
  [-7.28743, 112.67839],
  ...FD5_PTC_OUTER_CURVE,
  [-7.28706, 112.67847],
  [-7.28690, 112.67832],
];
const FD5_DUKUH_KUPANG_ENTER: LatLng = [-7.28315, 112.71106];
const FD5_DUKUH_KUPANG_DETOUR: LatLng = [-7.28132, 112.71163];
const FD5_DUKUH_KUPANG_EXIT: LatLng = [-7.28113, 112.71079];
const FD5_DUKUH_KUPANG_LOOP: LatLng[] = [
  [-7.28137, 112.71162],
  [-7.28109, 112.71170],
  [-7.28089, 112.71177],
  [-7.28102, 112.71220],
  [-7.28109, 112.71244],
  [-7.28154, 112.71233],
  [-7.28148, 112.71211],
  [-7.28139, 112.71173],
  [-7.28137, 112.71162],
];
const FD5_DARMO_PERMAI_ROAD: LatLng = [-7.27821, 112.69190];
const FD5_DARMO_PERMAI_SPUR_TIP: LatLng = [-7.27830, 112.69189];
const R2_BUNDARAN_UNESA_END: LatLng = [-7.29817, 112.67563];
const R2_UNESA_TERMINUS: LatLng = [-7.29830, 112.67540];
const R2_UNESA_UTURN_TAIL: LatLng[] = [
  [-7.29871, 112.67563],
  [-7.29874, 112.67565],
  [-7.29878, 112.67566],
  [-7.29881, 112.67566],
  [-7.29885, 112.67565],
  [-7.29888, 112.67563],
  [-7.29895, 112.67564],
  [-7.29930, 112.67566],
  [-7.29962, 112.67567],
  [-7.30006, 112.67570],
  [-7.30012, 112.67564],
  [-7.30014, 112.67561],
  [-7.30014, 112.67556],
  [-7.30009, 112.67549],
  [-7.29960, 112.67545],
  [-7.29924, 112.67543],
  [-7.29889, 112.67543],
  [-7.29886, 112.67540],
  [-7.29883, 112.67539],
  [-7.29879, 112.67539],
  [-7.29876, 112.67539],
  [-7.29872, 112.67541],
  [-7.29871, 112.67542],
  R2_UNESA_TERMINUS,
];
const R2_UNESA_START_HOOK: LatLng = [-7.29819, 112.67531];
const R2_UNESA_START_ROAD: LatLng = [-7.29819, 112.67539];
const R2_PRADAH_ROAD: LatLng = [-7.28425, 112.69188];
const R2_PRADAH_SPUR_TIP: LatLng = [-7.28448, 112.69207];
const R4_SIWALANKERTO_ENTER: LatLng = [-7.33575, 112.72942];
const R4_SIWALANKERTO_DETOUR: LatLng = [-7.33307, 112.73026];
const R4_SIWALANKERTO_EXIT: LatLng = [-7.33242, 112.73017];
const R4_SIWALANKERTO_FRONTAGE: LatLng[] = [
  [-7.33513, 112.72957],
  [-7.33447, 112.72972],
  [-7.33388, 112.72985],
  [-7.33376, 112.72987],
  [-7.33363, 112.72990],
  [-7.33333, 112.72996],
  [-7.33277, 112.73009],
];

export function patchSurabayaCorridorPoints(
  code: string,
  dir: Direction,
  pts: LatLng[],
): LatLng[] {
  let out = pts;

  if (code === "FD4" && dir === "b") {
    const spurIdx = findClosestInRange(out, FD4_ATOM1_SPUR_TIP, 0, out.length - 1, 15);
    if (spurIdx >= 0) {
      const enterStart = Math.max(0, spurIdx - 50);
      let enterIdx = -1;
      for (let i = enterStart; i < spurIdx; i++) {
        if (approxMeters(out[i], FD4_STASIUN_KOTA_JUNCTION) <= 10) {
          enterIdx = i;
          break;
        }
      }
      const exitEnd = Math.min(out.length - 1, spurIdx + 50);
      let exitIdx = -1;
      for (let i = exitEnd; i > spurIdx; i--) {
        if (approxMeters(out[i], FD4_STASIUN_KOTA_JUNCTION) <= 10) {
          exitIdx = i;
          break;
        }
      }
      if (enterIdx >= 0 && exitIdx > enterIdx) {
        const nextIdx = approxMeters(out[enterIdx], out[exitIdx]) <= 2 ? exitIdx + 1 : exitIdx;
        out = [...out.slice(0, enterIdx + 1), ...out.slice(nextIdx)];
      }
    }
    return out;
  }

  if (code === "FD4" && dir === "a") {
    const loopIdx = findClosestInRange(out, FD4_SIAGA_LOOP, 0, out.length - 1, 25);
    if (loopIdx >= 0) {
      const entryIdx = findClosestInRange(
        out,
        FD4_ATOM_NORTH_GATE,
        loopIdx - 15,
        loopIdx - 1,
        15,
      );
      const rejoinIdx = findClosestInRange(
        out,
        FD4_ATOM_SOUTH_EXIT,
        loopIdx + 1,
        loopIdx + 15,
        15,
      );
      if (entryIdx >= 0 && rejoinIdx > entryIdx) {
        out = [
          ...out.slice(0, entryIdx + 1),
          ...FD4_PASAR_ATOM_PATH,
          ...out.slice(rejoinIdx),
        ];
      }
    }
    return out;
  }

  if (code === "FD10" && dir === "a") {
    const loopIdx = findClosestInRange(out, FD4_SIAGA_LOOP, 0, out.length - 1, 25);
    if (loopIdx >= 0) {
      const entryIdx = findClosestInRange(
        out,
        FD10_BUNGURAN_NORTH_GATE,
        loopIdx - 15,
        loopIdx - 1,
        15,
      );
      if (entryIdx >= 0) {
        out = [
          ...out.slice(0, entryIdx + 1),
          FD4_ATOM_NORTH_GATE,
          ...FD4_PASAR_ATOM_PATH,
          FD10_ATOM2_TERMINUS,
        ];
      }
    }
    out = stripSpur(out, FD10_UNIPRA_SPUR_TIP, FD10_UNIPRA_ROAD);
    out = stripSpur(out, FD10_ITATS2_SPUR_TIP, FD10_ITATS2_ROAD);
    return out;
  }

  if (code === "FD10" && dir === "b") {
    if (approxMeters(out[0], FD4_ATOM1_SPUR_TIP) <= 15) {
      const startIdx = findClosestInRange(out, FD10_ATOM2_TERMINUS, 1, 50, 10);
      if (startIdx > 0) {
        out = out.slice(startIdx);
      }
    }
    out = stripSpur(out, FD10_RANGKAH_SPUR_TIP, FD10_RANGKAH_ROAD);
    out = stripSpur(out, FD10_SWK_SPUR_TIP, FD10_SWK_ROAD);
    out = stripSpur(out, FD10_NAROTAMA_SPUR_TIP, FD10_NAROTAMA_ROAD);
    out = stripSpur(out, FD10_PUSKESMAS_SPUR_TIP, FD10_PUSKESMAS_ROAD);
    out = stripSpur(out, FD10_SMPN19_SPUR_TIP, FD10_SMPN19_ROAD);
    const pacarIdx = findClosestInRange(out, FD10_PACAR_KELING_KINK, 0, out.length - 1, 5);
    if (pacarIdx >= 0) {
      out = [...out.slice(0, pacarIdx), FD10_PACAR_KELING_ROAD, ...out.slice(pacarIdx + 1)];
    }
    const marinaDetourIdx = findClosestInRange(out, FD10_MARINA_DETOUR_NW, 0, out.length - 1, 6);
    if (marinaDetourIdx >= 0) {
      const enterIdx = findClosestInRange(
        out,
        FD10_MARINA_ROAD_W,
        marinaDetourIdx - 4,
        marinaDetourIdx - 1,
        6,
      );
      const exitIdx = findClosestInRange(
        out,
        FD10_MARINA_ROAD_E,
        marinaDetourIdx + 1,
        marinaDetourIdx + 4,
        6,
      );
      if (enterIdx >= 0 && exitIdx > enterIdx) {
        out = [...out.slice(0, enterIdx + 1), ...out.slice(exitIdx)];
      }
    }
    return out;
  }

  if (code === "R5" && dir === "a") {
    out = stripSpur(out, R5_BENOWO_SPUR_TIP, R5_BENOWO_ROAD);
    out = stripSpur(out, R5_BIBIS_SPUR_TIP, R5_BIBIS_ROAD);
    const uturnIdx = findClosestInRange(out, R5_SIMO_UTURN_TURN, 0, out.length - 1, 6);
    if (uturnIdx >= 0) {
      const enterIdx = findClosestInRange(out, R5_SIMO_NORTH, uturnIdx - 6, uturnIdx - 1, 20);
      const exitIdx = findClosestInRange(out, R5_SIMO_NORTH, uturnIdx + 10, uturnIdx + 35, 20);
      if (enterIdx >= 0 && exitIdx > enterIdx) {
        const nextIdx = approxMeters(out[enterIdx], out[exitIdx]) <= 2 ? exitIdx + 1 : exitIdx;
        out = [...out.slice(0, enterIdx + 1), ...out.slice(nextIdx)];
      }
    }
    return out;
  }

  if (code === "FD12" && dir === "a") {
    const loopIdx = findClosestInRange(out, FD12_GUNUNG_ANYAR_LOOP, 0, out.length - 1, 15);
    if (loopIdx >= 0) {
      const enterIdx = findClosestInRange(
        out,
        FD12_GUNUNG_ANYAR_MERR,
        loopIdx - 45,
        loopIdx - 20,
        20,
      );
      const exitIdx = findClosestInRange(
        out,
        FD12_GUNUNG_ANYAR_MERR,
        loopIdx + 1,
        loopIdx + 20,
        20,
      );
      if (enterIdx >= 0 && exitIdx > enterIdx) {
        const nextIdx = approxMeters(out[enterIdx], out[exitIdx]) <= 2 ? exitIdx + 1 : exitIdx;
        out = [...out.slice(0, enterIdx + 1), ...out.slice(nextIdx)];
      }
    }
    return out;
  }

  if (code === "FD2" && dir === "b") {
    return replaceDetour(
      out,
      FD2_KARTINI_SKIP,
      FD2_TEUKU_UMAR_TURN,
      FD2_IMAM_BONJOL_REJOIN,
      FD2_WR_SUPRATMAN_PATH,
      { beforeSpan: 5, afterSpan: 5 },
    );
  }

  if (code === "FD3" && dir === "a") {
    return replaceDetour(out, FD3_SEMOLOWARU_JOG, FD3_SEMOLOWARU_CORNER, FD3_SEMOLOWARU_SOUTH, [], {
      detourMax: 5,
      enterMax: 5,
      exitMax: 5,
      beforeSpan: 3,
      afterSpan: 4,
    });
  }

  if (code === "FD3" && dir === "b") {
    return replaceDetour(
      out,
      FD3_WONOKROMO_DETOUR,
      FD3_WONOKROMO_ENTER,
      FD3_WONOKROMO_EXIT,
      FD3_WONOKROMO_MAIN,
      { detourMax: 5, enterMax: 8, exitMax: 8, beforeSpan: 6, afterSpan: 6 },
    );
  }

  if (code === "FD5" && dir === "a") {
    return replaceDetour(
      out,
      FD5_PTC_A_DETOUR,
      FD5_PTC_A_ENTER,
      FD5_PTC_A_EXIT,
      FD5_PTC_OUTER_CURVE,
      { detourMax: 4, enterMax: 8, exitMax: 8, beforeSpan: 4, afterSpan: 4 },
    );
  }

  if (code === "FD5" && dir === "b") {
    out = replaceDetour(
      out,
      FD5_DUKUH_KUPANG_DETOUR,
      FD5_DUKUH_KUPANG_ENTER,
      FD5_DUKUH_KUPANG_EXIT,
      FD5_DUKUH_KUPANG_LOOP,
      { detourMax: 6, enterMax: 10, exitMax: 10, beforeSpan: 3, afterSpan: 4 },
    );
    out = stripSpur(out, FD5_DARMO_PERMAI_SPUR_TIP, FD5_DARMO_PERMAI_ROAD);
    out = replaceDetour(
      out,
      FD5_PTC_B_DETOUR,
      FD5_PTC_B_ENTER,
      FD5_PTC_B_EXIT,
      FD5_PTC_B_OUTER_CURVE,
      { detourMax: 5, enterMax: 10, exitMax: 10, beforeSpan: 3, afterSpan: 4 },
    );
    return out;
  }

  if (code === "R2" && dir === "a") {
    out = stripSpur(out, R2_PRADAH_SPUR_TIP, R2_PRADAH_ROAD);
    if (approxMeters(out[out.length - 1], R2_BUNDARAN_UNESA_END) <= 15) {
      out = [...out, ...R2_UNESA_UTURN_TAIL];
    }
    return out;
  }

  if (code === "R2" && dir === "b") {
    if (
      approxMeters(out[0], R2_UNESA_START_HOOK) <= 8 &&
      approxMeters(out[1], R2_UNESA_START_ROAD) <= 8
    ) {
      return [R2_UNESA_TERMINUS, ...out.slice(1)];
    }
    return out;
  }

  if ((code === "R4" || code === "R1") && dir === "a") {
    return replaceDetour(
      out,
      R4_SIWALANKERTO_DETOUR,
      R4_SIWALANKERTO_ENTER,
      R4_SIWALANKERTO_EXIT,
      R4_SIWALANKERTO_FRONTAGE,
      { detourMax: 10, enterMax: 10, exitMax: 10, beforeSpan: 4, afterSpan: 4 },
    );
  }

  if (code === "FD8" && dir === "b") {
    return replaceDetour(out, FD8_JONO_SW_UTURN, FD8_JONO_NE_TURN, FD8_JONO_REJOIN, [], {
      beforeSpan: 16,
      afterSpan: 6,
    });
  }

  if (code === "FD9" && dir === "a") {
    out = replaceDetour(out, FD9_MENANGGAL_DIP, FD9_MENANGGAL_S, FD9_MENANGGAL_N, [], {
      detourMax: 8,
      enterMax: 8,
      exitMax: 8,
      beforeSpan: 3,
      afterSpan: 5,
    });
    out = stripSpur(out, FD9_MENANGGAL_SPUR_TIP, FD9_MENANGGAL_SPUR_ROAD);
    out = stripSpur(out, FD9_PAGESANGAN_SPUR_TIP, FD9_PAGESANGAN_ROAD);
    out = replaceDetour(out, FD9_DARMO_PARK_DIP, FD9_DARMO_PARK_E, FD9_DARMO_PARK_W, [], {
      detourMax: 8,
      enterMax: 8,
      exitMax: 8,
      beforeSpan: 4,
      afterSpan: 6,
    });
    out = replaceDetour(
      out,
      FD9_TANJUNGSARI_A_DETOUR,
      FD9_SATELIT_A_TURN,
      FD9_TANDES_A_REJOIN,
      FD9_DARMO_SATELIT_A,
      { detourMax: 20, enterMax: 15, exitMax: 15, beforeSpan: 10, afterSpan: 25 },
    );
    out = stripSpur(out, FD9_KARANGPOH_SPUR_TIP, FD9_KARANGPOH_ROAD);
    out = replaceDetour(
      out,
      FD9_BALONG_TIP,
      FD9_BALONG_N,
      FD9_BALONG_S,
      FD9_BALONG_BAY_A,
      { detourMax: 5, enterMax: 8, exitMax: 8, beforeSpan: 7, afterSpan: 7 },
    );
    return out;
  }

  if (code === "FD9" && dir === "b") {
    out = replaceDetour(
      out,
      FD9_BALONG_TIP,
      FD9_BALONG_S,
      FD9_BALONG_N,
      FD9_BALONG_BAY_B,
      { detourMax: 5, enterMax: 8, exitMax: 8, beforeSpan: 7, afterSpan: 7 },
    );
    out = replaceDetour(
      out,
      FD9_TANJUNGSARI_B_DETOUR,
      FD9_TANDES_B_TURN,
      FD9_SATELIT_B_REJOIN,
      FD9_DARMO_SATELIT_B,
      { detourMax: 20, enterMax: 15, exitMax: 15, beforeSpan: 20, afterSpan: 10 },
    );
    out = replaceDetour(out, FD9_ADITYA_DETOUR, FD9_ADITYA_ENTER, FD9_ADITYA_EXIT, [], {
      detourMax: 8,
      enterMax: 8,
      exitMax: 8,
      beforeSpan: 10,
      afterSpan: 12,
    });
    out = stripSpur(out, FD9_CILIWUNG_B_TIP, FD9_CILIWUNG_B_ROAD, 5);
    return out;
  }

  if (code === "FD13" && dir === "a") {
    return replaceDetour(out, YOS_SUDARSO_LOOP_W, YOS_SUDARSO_S, YOS_SUDARSO_N, [], {
      detourMax: 8,
      enterMax: 8,
      exitMax: 8,
      beforeSpan: 5,
      afterSpan: 6,
    });
  }

  if (code !== "FD7") return out;

  if (dir === "a") {
    out = replaceDetour(
      out,
      FD7_PEMUDA_LOOP_SW,
      FD7_PEMUDA_LOOP_START,
      FD7_PEMUDA_LOOP_E,
      [],
      { detourMax: 6, enterMax: 10, exitMax: 10, beforeSpan: 4, afterSpan: 5 },
    );
    out = stripSpur(out, FD7_PEMUDA_SPUR_TIP, FD7_PEMUDA_SPUR_ROAD);
    out = replaceDetour(out, YOS_SUDARSO_LOOP_W, YOS_SUDARSO_S, YOS_SUDARSO_N, [], {
      detourMax: 8,
      enterMax: 8,
      exitMax: 8,
      beforeSpan: 5,
      afterSpan: 6,
    });
    const koblenIdx = out.findIndex((p) => approxMeters(p, FD7_KOBLEN_DETOUR) <= 35);
    if (koblenIdx >= 0) {
      const entryIdx = findClosestInRange(
        out,
        FD7_BUBUTAN_ENTRY,
        koblenIdx - 20,
        koblenIdx - 1,
        35,
      );
      const rejoinIdx = findClosestInRange(
        out,
        FD7_PRINGADI_REJOIN,
        koblenIdx + 1,
        koblenIdx + 10,
        35,
      );
      if (entryIdx >= 0 && rejoinIdx >= 0 && entryIdx < rejoinIdx) {
        out = [...out.slice(0, entryIdx + 1), ...FD7_BUBUTAN_NORTH, ...out.slice(rejoinIdx)];
      }
    }
    out = replaceDetour(
      out,
      FD7_SUMBAWA_DETOUR,
      FD7_SUMBAWA_ENTER,
      FD7_SUMBAWA_EXIT,
      [],
      { detourMax: 8, enterMax: 8, exitMax: 8, beforeSpan: 6, afterSpan: 7 },
    );
    out = stripSpur(out, FD7_GENTENG_1_TIP, FD7_GENTENG_1_ROAD, 5);
    out = stripSpur(out, FD7_GENTENG_2_TIP, FD7_GENTENG_2_ROAD, 5);
    return out;
  }

  if (dir !== "b") return out;

  const detourIdx = out.findIndex((p) => approxMeters(p, FD7_GUBENG_DETOUR) <= 40);
  if (detourIdx >= 0) {
    const entryIdx = findClosestInRange(out, FD7_GUBENG_ENTRY, detourIdx - 30, detourIdx - 1, 35);
    const rejoinIdx = findClosestInRange(out, FD7_GUBENG_REJOIN, detourIdx + 1, detourIdx + 15, 35);
    if (entryIdx >= 0 && rejoinIdx >= 0 && entryIdx < rejoinIdx) {
      out = [...out.slice(0, entryIdx + 1), ...FD7_BANDA_EAST, ...out.slice(rejoinIdx)];
    }
  }

  out = stripSpur(out, FD7_MENUR_SPUR_TIP, FD7_MENUR_ROAD);

  const ngindenIdx = out.findIndex(
    (p, i) =>
      i < out.length - 1 &&
      approxMeters(p, FD7_NGINDEN_EAST) <= 15 &&
      approxMeters(out[i + 1], FD7_NGINDEN_WEST) <= 15,
  );
  if (ngindenIdx >= 0) {
    out = [...out.slice(0, ngindenIdx + 1), ...FD7_NGINDEN_UTURN, ...out.slice(ngindenIdx + 1)];
  }

  return out;
}
