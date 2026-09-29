#include <stdio.h>
#include <math.h>
#include <time.h>
int main(){ struct timespec a,b; clock_gettime(CLOCK_MONOTONIC,&a); volatile float s=0; float x=1.0001f;
 for(long i=0;i<400000000L;i++){ x = x*1.0000001f + 1e-9f; if((i&1023)==0) s+=sinf(x);} clock_gettime(CLOCK_MONOTONIC,&b);
 printf("%.2f s\n",(b.tv_sec-a.tv_sec)+(b.tv_nsec-a.tv_nsec)*1e-9); return 0;}
