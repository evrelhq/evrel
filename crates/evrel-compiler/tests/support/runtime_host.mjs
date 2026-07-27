import fs from "node:fs";
import vm from "node:vm";
import { pathToFileURL } from "node:url";

const request = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const resultPath = process.argv[3];
const observations = [];
const identities = new WeakMap();
const symbolIdentities = new Map();
let nextIdentity = 0;

const observer = Object.freeze({
    observe(label, ...values) {
        observations.push({
            kind: "observation",
            label: encode(label),
            values: values.map(encode),
        });
    },
});

Object.defineProperty(globalThis, "__evrel", {
    value: observer,
    configurable: true,
});

const consoleMethods = ["debug", "error", "info", "log", "warn"];
const capturedConsole = Object.fromEntries(
    consoleMethods.map((method) => [
        method,
        (...values) => {
            observations.push({
                kind: "console",
                method,
                values: values.map(encode),
            });
        },
    ]),
);
Object.defineProperty(globalThis, "console", {
    value: Object.freeze(capturedConsole),
    configurable: true,
});

let completion;
try {
    if (request.entryPath !== undefined) {
        await import(pathToFileURL(request.entryPath).href);
    } else if (request.mode === "module") {
        const encoded = Buffer.from(request.source).toString("base64");
        await import(`data:text/javascript;base64,${encoded}`);
    } else if (request.mode === "script") {
        vm.runInThisContext(request.source, { filename: "runtime-fixture.js" });
    } else {
        throw new Error(`unknown execution mode ${request.mode}`);
    }
    completion = { kind: "normal" };
} catch (error) {
    completion = { kind: "throw", value: encodeThrown(error) };
}

fs.writeFileSync(resultPath, JSON.stringify({ completion, observations }));

function encode(value) {
    if (value === null) return { type: "null" };

    switch (typeof value) {
        case "undefined":
            return { type: "undefined" };
        case "boolean":
        case "string":
            return { type: typeof value, value };
        case "number":
            if (Number.isNaN(value)) return { type: "number", value: "NaN" };
            if (Object.is(value, -0)) return { type: "number", value: "-0" };
            if (value === Infinity) return { type: "number", value: "Infinity" };
            if (value === -Infinity) return { type: "number", value: "-Infinity" };
            return { type: "number", value };
        case "bigint":
            return { type: "bigint", value: value.toString() };
        case "symbol":
            return {
                type: "symbol",
                identity: symbolIdentity(value),
                globalKey: Symbol.keyFor(value) ?? null,
                description: value.description ?? null,
            };
        case "function":
        case "object":
            return { type: typeof value, identity: objectIdentity(value) };
        default:
            throw new Error(`cannot encode value of type ${typeof value}`);
    }
}

function encodeThrown(value) {
    if (value instanceof Error) {
        return { type: "error", name: String(value.name) };
    }
    return encode(value);
}

function objectIdentity(value) {
    let identity = identities.get(value);
    if (identity === undefined) {
        identity = nextIdentity++;
        identities.set(value, identity);
    }
    return identity;
}

function symbolIdentity(value) {
    let identity = symbolIdentities.get(value);
    if (identity === undefined) {
        identity = nextIdentity++;
        symbolIdentities.set(value, identity);
    }
    return identity;
}
