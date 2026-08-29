<script setup lang="ts">
/**
 * 登录 / 注册 / 找回密码 / 改密码弹窗。
 *
 * 四种模式共用一个弹窗，因为它们之间要频繁互相跳转（"没有账号？去注册"、
 * "忘记密码？"、"已有账号？去登录"、"不记得原密码了？用短信重置"），
 * 做成四个独立弹窗会让状态在组件之间来回搬，反而更乱。
 *
 * 两个必须遵守的约束：
 * 1. **必须 Teleport 到 body**：父级 `.ai-card` 有 `overflow-y: auto`
 *    （style.css），不 teleport 会被裁掉一半。
 * 2. **token 永远不出现在这里**：它由 Rust 侧（src-tauri/src/session.rs）存盘，
 *    前端只知道"登没登录、用户名是什么"。少一条被注入脚本顺走的路径。
 *
 * 短信签名的问题也在这里兜：阿里云免资质通道的签名是服务商的名字，
 * 不会出现"灵魂灯笼"四个字，用户很容易当成诈骗短信忽略掉。所以发码之后
 * 必须显式告诉用户"短信来自谁"——这个提示不是客套话，是这条方案能不能用的关键。
 */
import { computed, nextTick, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { smsSignName } from "../logic/auth";

const props = withDefaults(
  defineProps<{
    /** 打开时停在哪一屏。改密码要能从账号区直接跳进来，不用先看到登录表单。 */
    initialMode?: Mode;
  }>(),
  { initialMode: "login" },
);

const open = defineModel<boolean>("open", { required: true });
const emit = defineEmits<{
  /** 登录/注册成功，父组件据此刷新登录态 */
  authed: [];
  toast: [message: string];
}>();

type Mode = "login" | "register" | "reset" | "change";
const mode = ref<Mode>("login");
const busy = ref(false);
const errorText = ref("");

// 登录
const account = ref("");
const loginPassword = ref("");

// 注册
const username = ref("");
const phone = ref("");
const password = ref("");
const confirmPassword = ref("");

// 已登录改密码。走这条**不发短信**——服务端 auth.rs 里那个函数的注释白纸黑字
// 写着"正常改密不该消耗短信配额"，而在补上这一屏之前，用户想改密码只能退出登录
// 去走"找回密码"，每改一次烧一条真实短信（约 0.042 元）。
const oldPassword = ref("");
const changePassword_ = ref("");
const changeConfirm = ref("");

// 找回密码
const resetPhone = ref("");
const resetPassword = ref("");
const resetConfirm = ref("");

// 验证码环节（注册和找回密码共用）
const codeSent = ref(false);
const code = ref("");
const phoneMasked = ref("");
/**
 * 短信开头方括号里那个名字。
 *
 * 【不要改成前端写死】它来自服务端 .env 里的 SMS_SIGN_NAME，以后换服务商就会变，
 * 写死在这儿等于界面开始骗人——而这段文案存在的唯一目的就是让用户相信那条
 * 陌生署名的短信是我们发的，说错了比不说更糟。拿不到就退回泛化说法。
 */
const signName = ref<string | null>(null);

/**
 * 界面上该显示哪个签名：优先用发码响应里带回来的那份（那是这条短信实际用的），
 * 没有就退回启动时从 /v1/version 拿到的那份。两处都拿不到就是 null。
 */
const shownSignName = computed(() => signName.value ?? smsSignName.value);
const logMode = ref(false);
const cooldown = ref(0);
let cooldownTimer: ReturnType<typeof setInterval> | undefined;

const firstInput = ref<HTMLInputElement | null>(null);

const TITLES: Record<Mode, string> = {
  login: "登录",
  register: "注册账号",
  reset: "找回密码",
  change: "修改密码",
};
const title = computed(() => TITLES[mode.value]);

function startCooldown(secs: number) {
  cooldown.value = secs;
  clearInterval(cooldownTimer);
  cooldownTimer = setInterval(() => {
    cooldown.value -= 1;
    if (cooldown.value <= 0) clearInterval(cooldownTimer);
  }, 1000);
}

function resetAll() {
  errorText.value = "";
  codeSent.value = false;
  code.value = "";
  phoneMasked.value = "";
  signName.value = null;
  logMode.value = false;
  cooldown.value = 0;
  clearInterval(cooldownTimer);
}

function switchTo(next: Mode) {
  mode.value = next;
  resetAll();
}

watch(open, async (isOpen) => {
  if (!isOpen) {
    clearInterval(cooldownTimer);
    return;
  }
  mode.value = props.initialMode;
  resetAll();
  await nextTick();
  firstInput.value?.focus();
});

/** 统一把后端抛上来的错误转成一句话。后端的中文文案是精心措过辞的
 *  （比如"用户名或密码不对"对"账号不存在"和"密码错了"故意给同一句，
 *  防的是账号枚举），这里原样展示，不要自作主张改写。 */
function describe(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

async function run<T>(fn: () => Promise<T>): Promise<T | undefined> {
  if (busy.value) return;
  busy.value = true;
  errorText.value = "";
  try {
    return await fn();
  } catch (err) {
    errorText.value = describe(err);
    return undefined;
  } finally {
    busy.value = false;
  }
}

interface CodeSent {
  phoneMasked: string;
  expiresInSecs: number;
  logMode: boolean;
  /** 短信签名，服务端下发。老服务端不发这个字段，所以是可选的。 */
  signName?: string | null;
}

async function doLogin() {
  const ok = await run(() =>
    invoke("auth_login", { account: account.value.trim(), password: loginPassword.value }),
  );
  if (ok !== undefined) {
    loginPassword.value = "";
    emit("authed");
    emit("toast", "已登录");
    open.value = false;
  }
}

async function doRegisterBegin() {
  const sent = await run(() =>
    invoke<CodeSent>("auth_register_begin", {
      username: username.value.trim(),
      password: password.value,
      confirmPassword: confirmPassword.value,
      phone: phone.value.trim(),
    }),
  );
  if (sent) {
    codeSent.value = true;
    phoneMasked.value = sent.phoneMasked;
    signName.value = sent.signName ?? null;
    logMode.value = sent.logMode;
    startCooldown(60);
  }
}

async function doResend() {
  if (cooldown.value > 0) return;
  const sent = await run(() =>
    mode.value === "register"
      ? invoke<CodeSent>("auth_register_resend", { phone: phone.value.trim() })
      : invoke<CodeSent>("auth_reset_begin", { phone: resetPhone.value.trim() }),
  );
  if (sent) {
    startCooldown(60);
    emit("toast", "验证码已重新发送");
  }
}

async function doRegisterVerify() {
  const ok = await run(() =>
    invoke("auth_register_verify", { phone: phone.value.trim(), code: code.value.trim() }),
  );
  if (ok !== undefined) {
    password.value = "";
    confirmPassword.value = "";
    emit("authed");
    emit("toast", "注册成功，已自动登录");
    open.value = false;
  }
}

async function doChangePassword() {
  const ok = await run(() =>
    invoke("auth_change_password", {
      oldPassword: oldPassword.value,
      newPassword: changePassword_.value,
      confirmPassword: changeConfirm.value,
    }),
  );
  if (ok !== undefined) {
    oldPassword.value = "";
    changePassword_.value = "";
    changeConfirm.value = "";
    // 服务端改密会吊销全部会话（含本机这条），所以改完就是登出状态，
    // 让父组件刷新一下，界面会回到"登录 / 注册"。
    emit("authed");
    emit("toast", "密码已修改，请用新密码重新登录");
    switchTo("login");
  }
}

async function doResetBegin() {
  const sent = await run(() =>
    invoke<CodeSent>("auth_reset_begin", { phone: resetPhone.value.trim() }),
  );
  if (sent) {
    codeSent.value = true;
    phoneMasked.value = sent.phoneMasked;
    signName.value = sent.signName ?? null;
    logMode.value = sent.logMode;
    startCooldown(60);
  }
}

async function doResetConfirm() {
  const ok = await run(() =>
    invoke("auth_reset_confirm", {
      phone: resetPhone.value.trim(),
      code: code.value.trim(),
      newPassword: resetPassword.value,
      confirmPassword: resetConfirm.value,
    }),
  );
  if (ok !== undefined) {
    resetPassword.value = "";
    resetConfirm.value = "";
    emit("toast", "密码已重置，请用新密码登录");
    switchTo("login");
  }
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") open.value = false;
}
</script>

<template>
  <Teleport to="body">
    <Transition name="modal-fade">
      <div v-if="open" class="modal-overlay" @click.self="open = false">
        <div class="modal-card auth-card" @keydown="onKeydown">
          <div class="auth-head">
            <h2>{{ title }}</h2>
            <button class="picker-close" type="button" aria-label="关闭" @click="open = false">×</button>
          </div>

          <!-- ---------------- 登录 ---------------- -->
          <div v-if="mode === 'login'" class="auth-form">
            <label class="auth-field">
              <span>用户名或手机号</span>
              <input
                ref="firstInput"
                v-model="account"
                autocomplete="username"
                spellcheck="false"
                @keydown.enter="doLogin"
              />
            </label>
            <label class="auth-field">
              <span>密码</span>
              <input
                v-model="loginPassword"
                type="password"
                autocomplete="current-password"
                @keydown.enter="doLogin"
              />
            </label>
            <button class="primary-btn auth-submit" type="button" :disabled="busy" @click="doLogin">
              {{ busy ? "登录中…" : "登录" }}
            </button>
            <div class="auth-links">
              <button type="button" class="auth-link" @click="switchTo('register')">还没有账号？注册</button>
              <button type="button" class="auth-link" @click="switchTo('reset')">忘记密码？</button>
            </div>
          </div>

          <!-- ---------------- 注册 ---------------- -->
          <div v-else-if="mode === 'register'" class="auth-form">
            <template v-if="!codeSent">
              <label class="auth-field">
                <span>用户名</span>
                <input
                  ref="firstInput"
                  v-model="username"
                  placeholder="2~24 个字符，可以用中文"
                  spellcheck="false"
                />
              </label>
              <label class="auth-field">
                <span>手机号</span>
                <input v-model="phone" placeholder="11 位中国大陆手机号" inputmode="numeric" />
              </label>
              <label class="auth-field">
                <span>密码</span>
                <input v-model="password" type="password" placeholder="至少 8 个字符" autocomplete="new-password" />
              </label>
              <label class="auth-field">
                <span>确认密码</span>
                <input
                  v-model="confirmPassword"
                  type="password"
                  autocomplete="new-password"
                  @keydown.enter="doRegisterBegin"
                />
              </label>
              <!-- 【发码之前就要说】这条不是补充说明，是这套短信方案能不能用的前提：
                   免资质通道的署名是服务商的名字，用户点完"获取验证码"收到一条
                   陌生公司发来的短信，第一反应是诈骗。事后再解释已经晚了。 -->
              <p class="auth-notice auth-sign-hint">
                <template v-if="shownSignName">
                  验证码短信开头会写 <strong>【{{ shownSignName }}】</strong>，不是「灵魂灯笼」——
                  那是我们接入的验证码通道的署名，认准它就对了。
                </template>
                <template v-else>
                  验证码短信开头写的是我们接入的验证码通道的署名，不是「灵魂灯笼」，
                  收到时别当成诈骗短信。
                </template>
              </p>
              <button class="primary-btn auth-submit" type="button" :disabled="busy" @click="doRegisterBegin">
                {{ busy ? "发送中…" : "获取验证码" }}
              </button>
              <div class="auth-links">
                <button type="button" class="auth-link" @click="switchTo('login')">已有账号？去登录</button>
              </div>
            </template>

            <template v-else>
              <p class="auth-hint">
                验证码已发送到 <strong>{{ phoneMasked }}</strong>，5 分钟内有效。
              </p>
              <!-- 这段提示是这条短信方案能不能用的关键，不是客套话：
                   阿里云个人免资质通道的短信签名是服务商的名字，不可能显示成
                   「灵魂灯笼」。不提前打招呼的话，用户看到一个完全陌生的公司名，
                   很可能当成诈骗短信直接忽略甚至举报。 -->
              <p v-if="shownSignName" class="auth-notice">
                短信开头写的是 <strong>【{{ shownSignName }}】</strong>，<strong>不是「灵魂灯笼」</strong>——
                那是我们接入的验证码通道的署名，认准这个名字就对了。
                如果一直收不到，检查一下手机管家 / 骚扰拦截里有没有被误拦。
              </p>
              <p v-else class="auth-notice">
                短信开头显示的是我们接入的短信服务商名称，<strong>不是「灵魂灯笼」</strong>——
                这是正规验证码通道，不是垃圾短信，请放心查收。
                如果一直收不到，检查一下手机管家 / 骚扰拦截里有没有被误拦。
              </p>
              <p v-if="logMode" class="auth-notice">
                服务器当前是<strong>日志模式</strong>，短信不会真的发出（这是维护状态，请联系作者）。
              </p>
              <label class="auth-field">
                <span>验证码</span>
                <input
                  ref="firstInput"
                  v-model="code"
                  inputmode="numeric"
                  placeholder="6 位数字"
                  @keydown.enter="doRegisterVerify"
                />
              </label>
              <button class="primary-btn auth-submit" type="button" :disabled="busy" @click="doRegisterVerify">
                {{ busy ? "验证中…" : "完成注册" }}
              </button>
              <div class="auth-links">
                <button type="button" class="auth-link" :disabled="cooldown > 0 || busy" @click="doResend">
                  {{ cooldown > 0 ? `重新发送（${cooldown}s）` : "重新发送验证码" }}
                </button>
                <button type="button" class="auth-link" @click="codeSent = false">改一下信息</button>
              </div>
            </template>
          </div>

          <!-- ---------------- 已登录改密码 ---------------- -->
          <!-- 这一屏的存在意义很具体：没有它，用户想改密码只能退出登录去走
               "找回密码"，每改一次烧一条真实短信。服务端那条路径本来就是为了
               不消耗短信配额而写的。 -->
          <div v-else-if="mode === 'change'" class="auth-form">
            <label class="auth-field">
              <span>原密码</span>
              <input
                ref="firstInput"
                v-model="oldPassword"
                type="password"
                autocomplete="current-password"
              />
            </label>
            <label class="auth-field">
              <span>新密码</span>
              <input
                v-model="changePassword_"
                type="password"
                placeholder="至少 8 个字符"
                autocomplete="new-password"
              />
            </label>
            <label class="auth-field">
              <span>确认新密码</span>
              <input
                v-model="changeConfirm"
                type="password"
                autocomplete="new-password"
                @keydown.enter="doChangePassword"
              />
            </label>
            <p class="auth-hint">改完会退出当前登录，需要用新密码重新登录一次。</p>
            <button class="primary-btn auth-submit" type="button" :disabled="busy" @click="doChangePassword">
              {{ busy ? "提交中…" : "修改密码" }}
            </button>
            <div class="auth-links">
              <button type="button" class="auth-link" @click="switchTo('reset')">不记得原密码了？用短信重置</button>
            </div>
          </div>

          <!-- ---------------- 找回密码 ---------------- -->
          <div v-else class="auth-form">
            <template v-if="!codeSent">
              <p class="auth-hint">输入注册时用的手机号，我们会发一条验证码短信。</p>
              <label class="auth-field">
                <span>手机号</span>
                <input
                  ref="firstInput"
                  v-model="resetPhone"
                  placeholder="11 位中国大陆手机号"
                  inputmode="numeric"
                  @keydown.enter="doResetBegin"
                />
              </label>
              <!-- 【发码之前就要说】这条不是补充说明，是这套短信方案能不能用的前提：
                   免资质通道的署名是服务商的名字，用户点完"获取验证码"收到一条
                   陌生公司发来的短信，第一反应是诈骗。事后再解释已经晚了。 -->
              <p class="auth-notice auth-sign-hint">
                <template v-if="shownSignName">
                  验证码短信开头会写 <strong>【{{ shownSignName }}】</strong>，不是「灵魂灯笼」——
                  那是我们接入的验证码通道的署名，认准它就对了。
                </template>
                <template v-else>
                  验证码短信开头写的是我们接入的验证码通道的署名，不是「灵魂灯笼」，
                  收到时别当成诈骗短信。
                </template>
              </p>
              <button class="primary-btn auth-submit" type="button" :disabled="busy" @click="doResetBegin">
                {{ busy ? "发送中…" : "获取验证码" }}
              </button>
              <div class="auth-links">
                <button type="button" class="auth-link" @click="switchTo('login')">想起来了，去登录</button>
              </div>
            </template>

            <template v-else>
              <p class="auth-hint">
                如果 <strong>{{ phoneMasked }}</strong> 是已注册的手机号，验证码已经发过去了。
              </p>
              <p class="auth-notice">
                短信开头显示的是短信服务商名称，<strong>不是「灵魂灯笼」</strong>，这是正规验证码通道。
              </p>
              <label class="auth-field">
                <span>验证码</span>
                <input v-model="code" inputmode="numeric" placeholder="6 位数字" />
              </label>
              <label class="auth-field">
                <span>新密码</span>
                <input v-model="resetPassword" type="password" placeholder="至少 8 个字符" autocomplete="new-password" />
              </label>
              <label class="auth-field">
                <span>确认新密码</span>
                <input
                  v-model="resetConfirm"
                  type="password"
                  autocomplete="new-password"
                  @keydown.enter="doResetConfirm"
                />
              </label>
              <button class="primary-btn auth-submit" type="button" :disabled="busy" @click="doResetConfirm">
                {{ busy ? "提交中…" : "重置密码" }}
              </button>
              <div class="auth-links">
                <button type="button" class="auth-link" :disabled="cooldown > 0 || busy" @click="doResend">
                  {{ cooldown > 0 ? `重新发送（${cooldown}s）` : "重新发送验证码" }}
                </button>
                <button type="button" class="auth-link" @click="switchTo('login')">去登录</button>
              </div>
            </template>
          </div>

          <p v-if="errorText" class="auth-error">{{ errorText }}</p>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>
