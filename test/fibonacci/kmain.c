// Copyright(c) The Maintainers of Nanvix.
// Licensed under the MIT License.

#define N 512

/**
 * @brief Next random number.
 */
static unsigned long next = 2;

/**
 * @brief Computes a random number.
 *
 * @param state Random number generator state.
 *
 * Compute x = (7^5 * x) mod (2^31 - 1) without overflowing 31 bits:
 *
 * (2^31 - 1) = 127773 * (7^5) + 2836
 *
 * From "Random number generators: good ones are hard to find", Park
 * and Miller, Communications of the ACM, vol. 31, no. 10, October
 * 1988, p. 1195.
 */
static int do_rand(unsigned long *state)
{
    long hi, lo, x;

    /* Must be in [1, 0x7ffffffe] range at this point. */
    hi = *state / 127773;
    lo = *state % 127773;
    x = 16807 * lo - 2836 * hi;

    if (x < 0)
        x += 0x7fffffff;
    *state = x;

    /* Transform to [0, 0x7ffffffd] range. */
    return (x - 1);
}

/**
 * @todo TODO: provide a detailed description for this function.
 */
void usrand(unsigned seed)
{
    next = seed;

    /* Transform to [1, 0x7ffffffe] range. */
    next = (next % 0x7ffffffe) + 1;
}

/**
 * @todo TODO: provide a detailed description for this function.
 */
int urand(void)
{
    return (do_rand(&next));
}

/**
 * @todo TODO: provide a detailed description for this function.
 */
int urand_r(unsigned *state)
{
    int r;
    unsigned long val;

    /* Transform to [1, 0x7ffffffe] range. */
    val = (*state % 0x7ffffffe) + 1;
    r = do_rand(&val);

    *state = (unsigned int)(val - 1);

    return (r);
}

// Create 3 static matrices N*N.
double a[N][N], b[N][N], c[N][N];

double kmain(void)
{
    usrand(1);

    // Initialize matrices.
    for (unsigned i = 0; i < N; i++) {
        for (unsigned j = 0; j < N; j++) {
            a[i][j] = urand() % 100;
            b[i][j] = urand() % 100;
            c[i][j] = 0;
        }
    }

    // Multiply matrices.
    for (unsigned i = 0; i < N; i++) {
        for (unsigned j = 0; j < N; j++) {
            for (unsigned k = 0; k < N; k++) {
                c[i][j] += a[i][k] * b[k][j];
            }
        }
    }

    return c[N - 1][N - 1];
}
