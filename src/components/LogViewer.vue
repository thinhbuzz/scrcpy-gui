<script lang="ts" setup>
import { ref, nextTick, watch } from "vue";
import { Textarea } from "ant-design-vue";

const props = defineProps<{
  logLines: string[];
}>();

const logRef = ref<any>(undefined);
const internalValue = ref("");

// Watch for changes in logLines to update the text area content
watch(
  () => props.logLines,
  (newLines) => {
    // Limit to last 1000 lines to prevent performance issues
    const validLines = newLines.slice(-1000);
    internalValue.value = validLines.join("");
    
    // Auto-scroll to bottom
    nextTick(() => {
      if (logRef.value && logRef.value.resizableTextArea) {
        const textArea = logRef.value.resizableTextArea.textArea;
        textArea.scrollTop = textArea.scrollHeight;
      }
    });
  },
  { deep: true }
);

defineExpose({
  scrollToBottom: () => {
    if (logRef.value && logRef.value.resizableTextArea) {
      const textArea = logRef.value.resizableTextArea.textArea;
      textArea.scrollTop = textArea.scrollHeight;
    }
  }
});
</script>

<template>
  <div class="log-container common-box flex-item">
    <h3>Logs</h3>
    <div class="log-scroller">
      <Textarea
        :rows="20"
        ref="logRef"
        :value="internalValue"
        :readonly="true"
        :autoSize="false"
        class="log-textarea"
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
  margin-bottom: 10px;
}
</style>
