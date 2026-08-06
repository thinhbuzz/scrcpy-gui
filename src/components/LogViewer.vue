<script lang="ts" setup>
import { ref, nextTick, watch, computed } from "vue";
import { Textarea } from "ant-design-vue";

const props = defineProps<{
  logLines: string[];
  title?: string;
}>();

// ant-design-vue Textarea exposes `resizableTextArea` on its component ref
// for imperative access to the underlying <textarea> element.
const logRef = ref<{
  resizableTextArea?: { textArea: HTMLTextAreaElement };
} | null>(null);

// Use a computed property to join log lines, limited to the last 1000 lines
const internalValue = computed(() => props.logLines.slice(-1000).join(""));

// Watch the joined content so auto-scroll works even after the array is
// pinned at maxLogLines (1000) — the length stops changing but content doesn't.
watch(
  () => internalValue.value,
  () => {
    nextTick(() => {
      scrollToBottom();
    });
  }
);

const scrollToBottom = () => {
  const instance = logRef.value;
  if (instance && instance.resizableTextArea) {
    const textArea = instance.resizableTextArea.textArea;
    textArea.scrollTop = textArea.scrollHeight;
  }
};
</script>

<template>
  <div class="log-container">
    <h3 v-if="props.title !== ''">{{ props.title ?? "Logs" }}</h3>
    <div class="log-scroller">
      <Textarea
        :rows="20"
        ref="logRef"
        :value="internalValue"
        :readonly="true"
        :autoSize="false"
        class="log-textarea"
        aria-label="Log output"
      ></Textarea>
    </div>
  </div>
</template>

<style lang="scss" scoped>
.log-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  .log-scroller {
    flex-grow: 1;
    overflow-x: hidden;
    overflow-y: auto;
    
    .log-textarea {
      resize: none;
      font-family: monospace;
      white-space: pre-wrap;
    }
  }
}
h3 {
  margin: 0;
}
</style>
