import type { Composition, PlannedGroup } from "./composition-types.ts";
/** Edit existing planned marks in one named subject, without consulting the planner. */
export function editRegion(d: Composition, region: string, edit: (group: PlannedGroup) => void): Composition {
  const out = structuredClone(d);
  let found = false;
  for (const g of out.groups)
    if (g.region === region) {
      edit(g);
      found = true;
    }
  if (!found) throw new Error(`UNKNOWN_REGION: ${region}`);
  out.sourceHash = null;
  return out;
}
/** Accept only one region from a newly generated candidate plan. Other groups are byte-for-byte preserved.
 * Adding passes/groups or changing canvas settings requires a full composition replacement. */
export function replaceRegion(d: Composition, candidate: Composition, region: string): Composition {
  if (
    d.engine !== candidate.engine ||
    JSON.stringify(d.aspect) !== JSON.stringify(candidate.aspect) ||
    JSON.stringify(d.ground) !== JSON.stringify(candidate.ground)
  )
    throw new Error("REPLAN_SCOPE_CHANGED: canvas or engine differs");
  const ids = new Set(d.groups.filter((g) => g.region === region).map((g) => g.id));
  if (!ids.size) throw new Error(`UNKNOWN_REGION: ${region}`);
  const replacement = new Map(candidate.groups.filter((g) => g.region === region).map((g) => [g.id, g]));
  if ([...replacement.keys()].some((id) => !ids.has(id)))
    throw new Error("REPLAN_SCOPE_CHANGED: selected region has new passes; use a full replacement");
  return editRegion(d, region, (g) => {
    const next = replacement.get(g.id);
    if (next) {
      if (next.hblurSigma !== g.hblurSigma || next.dryAfter !== g.dryAfter)
        throw new Error("REPLAN_SCOPE_CHANGED: pass boundary operations differ");
      g.strokes = structuredClone(next.strokes);
    } else g.strokes = [];
  });
}
