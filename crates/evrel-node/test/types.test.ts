import {
  compile,
  compileProgram,
  compileSync,
  ModuleRequestKind,
  ResolvedModuleTargetKind,
  type CompileOptions,
  type CompileOutput,
  type ProgramInput,
  type ProgramOutput,
} from "../index.js";

declare const source: string;

const options: CompileOptions = {
  filename: "input.ts",
};

const compiled: Promise<CompileOutput> = compile(source, options);
const compiledSync: CompileOutput = compileSync(source, options);

const program: ProgramInput = {
  modules: [
    {
      key: "file:///entry.js",
      filename: "entry.js",
      source: "console.log(42);",
      resolvedRequests: [],
    },
  ],
  entrypoints: ["file:///entry.js"],
};

const compiledProgram: Promise<ProgramOutput> = compileProgram(program);

void [
  compiled,
  compiledSync,
];
void compiledProgram;
void ModuleRequestKind.StaticImport;
void ResolvedModuleTargetKind.Internal;
