// Barrel — re-exports every domain module so existing `import { x } from "../api"`
// call sites keep working unchanged (the old monolithic api.ts was split into
// per-domain modules; this index sits at the same import path).
export * from "./auth";
export * from "./server-url";
export * from "./license";
export * from "./catalog";
export * from "./reports";
export * from "./rubro";
export * from "./pos";
export * from "./cash";
export * from "./settings";
export * from "./customers";
export * from "./credit";
export * from "./purchases";
export * from "./expenses";
export * from "./prescriptions";
export * from "./dte";
export * from "./audit";
export * from "./seed";
export * from "./assist";
export * from "./print";
