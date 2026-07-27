const assert = require("node:assert/strict");
const test = require("node:test");

const {
  compile,
  compileProgram,
  compileSync,
} = require("../index.js");

test("infers TypeScript from the filename asynchronously", async () => {
  const output = await compile("console.log(42 satisfies number);", {
    filename: "component.tsx",
  });

  assert.match(output.code, /console/);
  assert.doesNotMatch(output.code, /satisfies number/);
});

test("compiles TSX based on the filename", async () => {
  const output = await compile(
    `
      const UI = {};
      const properties: object = {};
      <UI.Button enabled {...properties}>{20 + 22}</UI.Button>;
    `,
    { filename: "component.tsx" },
  );

  assert.match(output.code, /\.Button/);
  assert.match(output.code, /enabled/);
  assert.match(output.code, /\{\.\.\.[^}]+\}/);
  assert.match(output.code, /20 \+ 22/);
  assert.doesNotMatch(output.code, /: object/);
});

test("infers CommonJS from the filename synchronously", () => {
  const output = compileSync("return;", {
    filename: "module.cjs",
  });

  assert.deepEqual(output, {
    code: "",
  });
});

test("compiles a resolved multi-module program", async () => {
  const dependencyKey = "file:///dependency.js";
  const entryKey = "file:///entry.js";

  const output = await compileProgram({
    modules: [
      {
        key: dependencyKey,
        filename: "dependency.js",
        source: "export const answer = 40 + 2;",
        resolvedRequests: [],
      },
      {
        key: entryKey,
        filename: "entry.js",
        source:
          "import { answer } from './dependency.js'; console.log(answer);",
        resolvedRequests: [
          {
            kind: "staticImport",
            specifier: "./dependency.js",
            attributes: [],
            target: {
              kind: "internal",
              key: dependencyKey,
            },
          },
        ],
      },
    ],
    entrypoints: [entryKey],
  });

  assert.deepEqual(
    output.modules.map(({ key }) => key),
    [dependencyKey, entryKey],
  );
  assert.match(output.modules[0].code, /export const answer/);
  assert.match(output.modules[1].code, /import \{ answer \}/);
  assert.match(output.modules[1].code, /console/);
});

test("rejects invalid source asynchronously", async () => {
  await assert.rejects(compile("const = ;", { filename: "input.js" }));
});

test("throws for invalid source synchronously", () => {
  assert.throws(() => compileSync("const = ;", { filename: "input.js" }));
});

test("rejects unknown source extensions", async () => {
  await assert.rejects(compile("20 + 22;", { filename: "input.txt" }));
  assert.throws(() => compileSync("20 + 22;", { filename: "input.txt" }));
});
