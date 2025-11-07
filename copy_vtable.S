.section .text
.global copy_array

// Copy an array from source to destination
// Arguments:
//   r0: Destination address
//   r1: Source address
//   r2: Number of elements to copy
copy_array:
    cmp r2, #0
    beq copy_done

copy_loop:
    ldr r3, [r1]
    str r3, [r0]
	add r1, #4
    add r0, #4
    sub r2, #1
    bne copy_loop

copy_done:
    bx lr
