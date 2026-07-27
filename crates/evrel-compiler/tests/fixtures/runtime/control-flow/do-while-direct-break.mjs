let count = 0;

do {
    count++;
    break;
} while (count < 3);

__evrel.observe("do while direct break", count);
