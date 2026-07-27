globalThis.evrelIndirectEvalValue = 40;

function run() {
    const local = 2;
    const indirect = eval;
    return indirect("evrelIndirectEvalValue + (typeof local === 'undefined' ? 2 : 100)");
}

const result = run();
delete globalThis.evrelIndirectEvalValue;
__evrel.observe("indirect eval", result, typeof globalThis.evrelIndirectEvalValue);
