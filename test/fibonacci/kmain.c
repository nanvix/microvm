// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

void kmain(void)
{
    /* Computes the Fibonacci number, */
    unsigned long long a = 0, b = 1, c;
    for (int i = 0; i < 1000; i++) {
        c = a + b;
        a = b;
        b = c;
    }
}
