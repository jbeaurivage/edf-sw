.section .text
.global copy_vector_table

// Function to copy an array from source to destination
// Arguments:
//   r0: Destination address
//   r1: Source address
//   r2: Number of elements to copy
copy_vector_table:
    cmp r2, #0              // Check if the number of elements is zero
    beq copy_done           // If zero, we're done

copy_loop:
    ldr r3, [r1]        // Load a word from the source and increment the source pointer
    str r3, [r0]        // Store the word to the destination and increment the destination pointer
	add r1, #4          // Increment the source pointer
    add r0, #4  
    sub r2, #1         // Decrement the counter
    bne copy_loop           // Repeat until the counter reaches zero

copy_done:
    bx lr
