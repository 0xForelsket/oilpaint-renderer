#include <omp.h>
#include <stdio.h>
int main(){
#pragma omp parallel
{printf("t%d\n",omp_get_thread_num());}
return 0;}
