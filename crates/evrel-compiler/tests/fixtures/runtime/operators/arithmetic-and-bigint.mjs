const numbers = [
    7 + 5,
    7 - 5,
    7 * 5,
    7 / 2,
    7 % 5,
    2 ** 5,
];
const bigints = [7n + 5n, 7n - 5n, 7n * 5n, 7n / 2n, 7n % 5n, 2n ** 5n];

__evrel.observe("number arithmetic", ...numbers);
__evrel.observe("bigint arithmetic", ...bigints);
