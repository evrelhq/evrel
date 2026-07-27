globalThis.evrelOptionalEvalName = "global";

function run() {
    const evrelOptionalEvalName = "local";
    const evaluated = eval?.("evrelOptionalEvalName");
    return [evaluated, evrelOptionalEvalName];
}

const result = run();
delete globalThis.evrelOptionalEvalName;
__evrel.observe("optional eval indirectness", result[0], result[1]);
