globalThis.evrelSequenceEvalGlobal = 40;

function run() {
    const local = 2;
    const evaluated = (0, eval)(
        "evrelSequenceEvalGlobal + (typeof local === 'undefined' ? 2 : 100)",
    );
    return [evaluated, local];
}

const result = run();
delete globalThis.evrelSequenceEvalGlobal;
__evrel.observe("sequence indirect eval", result[0], result[1]);
