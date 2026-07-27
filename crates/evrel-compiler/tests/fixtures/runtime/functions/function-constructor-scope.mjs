globalThis.evrelFunctionConstructorValue = 40;

function create() {
    const local = 100;
    return Function(
        "offset",
        "return evrelFunctionConstructorValue + offset + (typeof local === 'undefined' ? 0 : local);",
    );
}

const generated = create();
const result = generated(2);
delete globalThis.evrelFunctionConstructorValue;
__evrel.observe("function constructor scope", result, generated.length);
