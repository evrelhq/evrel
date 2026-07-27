import { readA, valueA } from "./a.mjs";
import { readB, valueB } from "./b.mjs";

__evrel.observe("cycle", valueA, valueB, readA(), readB());
