// oilpaint: offline oil painting for agents and people. L2 ships the scene DSL, the generated ScenePlan types and
// the engine's validator and guide compiler (WASM); planning and painting follow in L3-L4.
export * from "./scene.ts";
export { loadEngine, OilError, MIXERS } from "./engine.ts";
export type { Engine, Guides, GuidesSummary, MixerId, OilIssue, Plan, PlanReport, PreviewName, RgbImage, Validation } from "./engine.ts";

export * from './author.ts';
