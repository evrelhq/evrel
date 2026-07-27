const object = { value: 42 };
const values = [object?.value, null?.value];

__evrel.observe("optional chain array element", values[0], values[1]);
