let binding = 3;
const bindingPostfix = binding--;
const bindingPrefix = --binding;

const object = { value: 3 };
const propertyPostfix = object.value--;
const propertyPrefix = --object.value;

__evrel.observe(
    "decrement updates",
    bindingPostfix,
    bindingPrefix,
    binding,
    propertyPostfix,
    propertyPrefix,
    object.value,
);
