import { ref } from "vue";

/**
 * 记住「是哪个按钮打开了弹窗」，供 ModalShell 做从该按钮流出来 / 收回去的展开动效。
 *
 * 同一时刻只会开一个弹窗，所以一个组件里的多个弹窗可以共用一个 origin：
 * 每次点击都会把当前按钮覆盖进去，正在开的那个弹窗读到的就是它自己的触发按钮。
 *
 *   const { origin, capture } = useModalOrigin();
 *   function openColor(event: MouseEvent) { capture(event); modalOpen.value = true; }
 *   <button @click="openColor">颜色</button>
 *   <ModalShell :origin="origin" ... />
 */
export function useModalOrigin() {
  const origin = ref<HTMLElement | null>(null);

  /** 从事件里取出触发元素。传 null 可清空（退化成淡入淡出）。 */
  function capture(event?: Event | null) {
    origin.value = (event?.currentTarget as HTMLElement | null) ?? null;
  }

  return { origin, capture };
}
