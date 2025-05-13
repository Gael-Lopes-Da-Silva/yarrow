#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>

// STRING
typedef struct {
    char* content;
    int size;
} String;

String* string_new(const char* content) {
    if (!content) return NULL;
    String* str = (String*)malloc(sizeof(String));
    if (!str) return NULL;
    str->content = strdup(content);
    if (!str->content) {
        free(str);
        return NULL;
    }
    str->size = strlen(content);
    return str;
}

int string_free(String* str) {
    if (!str) return -1;
    free(str->content);
    str->content = NULL;
    free(str);
    return 0;
}

String* string_slice(String* str, size_t start, size_t end) {
    if (!str || !str->content) return NULL;
    if (end == (size_t)-1) end = str->size;
    if (start > end || end > str->size) return NULL;

    size_t slice_len = end - start;
    char* slice_content = (char*)malloc(slice_len + 1);
    if (!slice_content) return NULL;

    strncpy(slice_content, str->content + start, slice_len);
    slice_content[slice_len] = '\0';

    String* slice = string_new(slice_content);
    free(slice_content);
    return slice;
}

String* string_join(String* str1, String* str2) {
    if (!str1 || !str1->content || !str2 || !str2->content) return NULL;

    size_t total_len = str1->size + str2->size;
    char* joined_content = (char*)malloc(total_len + 1);
    if (!joined_content) return NULL;

    strcpy(joined_content, str1->content);
    strcat(joined_content, str2->content);

    String* joined = string_new(joined_content);
    free(joined_content);
    return joined;
}


// DEFER
#define DEFER_STACK_NAME defer_stack
#define DEFER_MAX_STACK_SIZE 64

#define DEFER_FUNC_NAME_(line) defer_func_##line
#define DEFER_FUNC_NAME(line) DEFER_FUNC_NAME_(line)

typedef void (*DeferFunction)(void);

typedef struct {
    DeferFunction fn;
} DeferAction;

typedef struct {
    DeferAction actions[DEFER_MAX_STACK_SIZE];
    size_t count;
} DeferStack;

#define defer_init() \
    static DeferStack DEFER_STACK_NAME = { .count = 0 }; \

#define defer_add(call) \
    do { \
        if (DEFER_STACK_NAME.count < DEFER_MAX_STACK_SIZE) { \
            if (DEFER_STACK_NAME.count >= DEFER_MAX_STACK_SIZE) { \
                fprintf(stderr, "Warning: Defer stack overflow\n"); \
                break; \
            } \
            void DEFER_FUNC_NAME(__LINE__)(void) { call; } \
            DEFER_STACK_NAME.actions[DEFER_STACK_NAME.count].fn = DEFER_FUNC_NAME(__LINE__); \
            DEFER_STACK_NAME.count++; \
        } \
    } while (0)

#define defer_run() \
    do { \
        while (DEFER_STACK_NAME.count > 0) { \
            DEFER_STACK_NAME.count--; \
            if (DEFER_STACK_NAME.actions[DEFER_STACK_NAME.count].fn) { \
                DEFER_STACK_NAME.actions[DEFER_STACK_NAME.count].fn(); \
            } \
        } \
    } while (0)


// UTILS
char* file_read(const char* filename) {
    FILE* file = fopen(filename, "r");
    if (!file) return NULL;

    fseek(file, 0, SEEK_END);
    long file_size = ftell(file);
    fseek(file, 0, SEEK_SET);

    char* content = (char*)malloc(file_size + 1);
    if (!content) {
        fclose(file);
        return NULL;
    }

    size_t bytes_read = fread(content, 1, file_size, file);
    content[bytes_read] = '\0';

    fclose(file);

    return content;
}

int file_write(const char* filename, const char* content) {
    FILE* file = fopen(filename, "w");
    if (!file) return -1;

    size_t bytes_written = fwrite(content, 1, strlen(content), file);

    fclose(file);

    if (bytes_written != strlen(content)) {
        return -1;
    }

    return 0;
}

int file_append(const char* filename, const char* content) {
    FILE* file = fopen(filename, "a");
    if (!file) return -1;

    size_t bytes_written = fwrite(content, 1, strlen(content), file);

    fclose(file);

    if (bytes_written != strlen(content)) {
        return -1;
    }

    return 0;
}
