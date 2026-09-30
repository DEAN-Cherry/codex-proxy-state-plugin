// 仅用于本地验收的宿主替身，实际启动已编译插件，不连接生产账号或数据库。
import { fileURLToPath } from 'node:url'

export class PluginPeer {
  #process
  #pending = new Map()
  #nextId = 1
  #buffer = Buffer.alloc(0)
  #ready = Promise.withResolvers()
  #manifest
  states = new Map()
  storageError = false
  unavailableAccounts = new Set(['acct_demo_unavailable'])
  disabledAccounts = new Set(['acct_demo_plus_02'])
  accountNames = new Map([
    ['acct_demo_001', '主力账号'],
    ['acct_demo_002', '主力账号'],
    ['acct_demo_003', '研发专用'],
    ['acct_demo_004', ''],
    ['acct_demo_pro_01', 'Pro 备用'],
    ['acct_demo_plus_02', 'Plus 暂停'],
    ['acct_demo_empty_03', '尚未使用'],
    ['acct_demo_unavailable', '读取异常'],
  ])
  accounts = [
    'acct_demo_pro_01', 'acct_demo_plus_02', 'acct_demo_empty_03', 'acct_demo_unavailable',
    ...Array.from({ length: 52 }, (_, index) => `acct_demo_${String(index + 1).padStart(3, '0')}`),
  ]

  constructor(manifest) {
    this.#manifest = manifest
    const executable = process.env.QA_PLUGIN_BINARY ?? fileURLToPath(new URL(
      `../backend/target/debug/codex-proxy-state-plugin${process.platform === 'win32' ? '.exe' : ''}`,
      import.meta.url,
    ))
    this.#process = Bun.spawn([executable], { stdin: 'pipe', stdout: 'pipe', stderr: 'inherit' })
    this.#read().catch(error => this.#fail(error))
    this.#process.exited.then(code => this.#fail(new Error(`plugin exited ${code}`)))
  }

  async start() {
    const contributes = Object.fromEntries(Object.entries(this.#manifest.contributes).map(([name, declaration]) => [
      name, {
        id: `${this.#manifest.publisher}.${this.#manifest.name}.${name.replaceAll('_', '-')}`,
        version: name === 'middleware' ? 3 : 1,
        stages: name === 'middleware' ? declaration.stages : [name === 'management' ? 'management' : 'observation'],
        ...declaration,
      },
    ]))
    this.#send({
      type: 'hello',
      handshake: {
        protocol_version: 2, artifact_sha256: 'a'.repeat(64),
        plugin_id: `${this.#manifest.publisher}.${this.#manifest.name}`,
        instance_id: 'qa-state-observer', generation: 1, incarnation: 'qa-process',
        configuration: {}, contributes,
      },
    })
    await this.#ready.promise
    const registration = await this.call('plugin.register', 'registration', {}, Buffer.alloc(0))
    if (registration.message.type !== 'result') throw new Error('registration failed')
    const registered = registration.message.result.contributes
    if (registered.middleware?.version !== 3 ||
        registered.observer?.stages?.[0] !== 'observation' ||
        Object.keys(registered).sort().join(',') !== 'management,middleware,observer')
      throw new Error('registration lacks protocol-2 middleware and observer capabilities')
    return this
  }

  call(method, stage, params, payload = Buffer.alloc(0), callback, requestId = 'qa-management') {
    const id = this.#nextId
    this.#nextId += 2
    const result = Promise.withResolvers()
    const timeout = setTimeout(() => {
      this.#pending.delete(id)
      this.#send({ type: 'cancel', id })
      result.reject(new Error(`plugin call timed out: ${method}`))
    }, 10_000)
    this.#pending.set(id, { ...result, timeout, callback })
    this.#send({
      type: 'call', id, method,
      context: {
        call_id: id, instance_id: 'qa-state-observer', generation: 1, incarnation: 'qa-process',
        stage, timeout_ms: 10_000, resource_stream: false, resource_scope_id: 'qa-scope', request_id: requestId,
        account_id: params.account_id,
      },
      params,
    }, payload)
    if (method === 'middleware.handle') {
      this.#send({ type: 'credit', id, bytes: 1024 * 1024, frames: 128 })
    }
    return result.promise
  }

  async api(method, path, body, query = '') {
    const frame = await this.call('management.handle', 'management', {
      method, path, query, content_type: body === undefined ? null : 'application/json',
    }, body === undefined ? Buffer.alloc(0) : Buffer.from(JSON.stringify(body)))
    return { status: frame.message.result.status, value: JSON.parse(frame.payload.toString()) }
  }

  async observe(account, model, length) {
    const id = crypto.randomUUID()
    await this.call('middleware.handle', 'request', {
      request_id: id, mount: 'request', operation: 'generate', protocol: 'openai',
      endpoint: '/v1/responses', transport: 'http_sse', model, headers: [],
      settings_sources: {}, client_key_id: 'qa-key', account_group_ids: [],
    }, Buffer.from(JSON.stringify({ model, input: 'synthetic QA request', stream: true })),
    method => {
      if (method !== 'host.middleware.next') throw new Error(`unexpected callback ${method}`)
      return { result: {
        response: `response-${id}`, protocol: 'openai', status: 200,
        headers: [{ name: 'x-codex-turn-state', value: [...Buffer.from('Q'.repeat(length))] }],
      }, payload: Buffer.alloc(0) }
    }, id)
    await this.call('observer.observe', 'observation', { event: 'request_completed', data: {
      event_id: id, request_id: id, config_revision: 1, operation: 'generate',
      account_id: account, upstream_model: model, provider: 'openai', completed_at_ms: Date.now(),
      terminal: { outcome: 'succeeded', send_state: 'sent', attempt_count: 1 }, usage: {},
    } }, Buffer.alloc(0), undefined, id)
  }

  async observeWebsocket(account, model, length) {
    const id = crypto.randomUUID()
    const response = await this.call('middleware.handle', 'attempt', {
      request_id: id, mount: 'attempt', attempt_index: 1, operation: 'generate', protocol: 'openai',
      endpoint: '/v1/responses', transport: 'web_socket', provider: 'openai', model,
      account_id: account, headers: [],
      settings_sources: {}, client_key_id: 'qa-key', account_group_ids: [],
    }, Buffer.from(JSON.stringify({ model: 'public-alias', input: 'synthetic QA request' })),
    method => {
      if (method !== 'host.middleware.next') throw new Error(`observer attempted body access: ${method}`)
      return { result: {
        response: `response-${id}`, protocol: 'openai', status: 200, headers: [],
        body: { handle: `body-${id}`, framing: 'json_document' },
      }, payload: Buffer.alloc(0) }
    }, id)
    if (response.message.result.body.kind !== 'pass_through') throw new Error('observer took stream ownership')
    await this.call('observer.observe', 'observation', { event: 'websocket_response', data: {
      event_id: id, request_id: id, config_revision: 1, operation: 'generate', protocol: 'openai',
      provider: 'openai', attempt_index: 1, sequence: 1, payload_included: true,
      requested_model: 'public-alias', account_id: account, event_type: 'codex.response.metadata',
    } }, Buffer.from(JSON.stringify({ type: 'codex.response.metadata', headers: { 'x-codex-turn-state': 'Q'.repeat(length) } })), undefined, id)
  }

  #send(message, payload = Buffer.alloc(0)) {
    const metadata = Buffer.from(JSON.stringify(message))
    const header = Buffer.alloc(12)
    header.writeUInt32BE(metadata.length)
    header.writeBigUInt64BE(BigInt(payload.length), 4)
    this.#process.stdin.write(Buffer.concat([header, metadata, payload]))
    this.#process.stdin.flush()
  }

  async #read() {
    for await (const chunk of this.#process.stdout) {
      this.#buffer = Buffer.concat([this.#buffer, chunk])
      while (this.#buffer.length >= 12) {
        const metadataBytes = this.#buffer.readUInt32BE()
        const payloadBytes = Number(this.#buffer.readBigUInt64BE(4))
        if (metadataBytes > 65_536 || payloadBytes > 8 * 1024 * 1024) throw new Error('oversized QA frame')
        const size = 12 + metadataBytes + payloadBytes
        if (this.#buffer.length < size) break
        const message = JSON.parse(this.#buffer.subarray(12, 12 + metadataBytes).toString())
        const payload = Buffer.from(this.#buffer.subarray(12 + metadataBytes, size))
        this.#buffer = this.#buffer.subarray(size)
        this.#dispatch({ message, payload }).catch(error => this.#fail(error))
      }
    }
  }

  async #dispatch(frame) {
    const message = frame.message
    if (message.type === 'ready') { this.#ready.resolve(); return }
    if (message.type === 'callback') {
      const parent = this.#pending.get(message.parent_id)
      if (!parent) throw new Error('callback without live parent')
      const reply = message.method.startsWith('host.middleware.')
        ? await parent.callback(message.method, message.params, frame.payload)
        : this.#host(message.method, message.params, frame.payload)
      this.#send(reply.error
        ? { type: 'error', id: message.id, error: reply.error }
        : { type: 'result', id: message.id, result: reply.result }, reply.payload)
      return
    }
    if (message.type === 'result' || message.type === 'error') {
      const call = this.#pending.get(message.id)
      if (!call) throw new Error('reply without pending call')
      clearTimeout(call.timeout)
      this.#pending.delete(message.id)
      if (message.type === 'error') call.reject(new Error(`plugin fault: ${message.error.code}`))
      else call.resolve(frame)
    }
  }

  #host(method, params, payload) {
    const fail = code => ({ error: { code, message: 'synthetic QA host error', send_state: 'not_sent' }, payload: Buffer.alloc(0) })
    if (this.storageError) return fail('fault')
    const key = `${params.namespace}:${params.key}`
    switch (method) {
      case 'host.state.get':
        if (this.unavailableAccounts.has(params.key)) return fail('fault')
        return { result: { record: this.states.get(key) ?? null }, payload: Buffer.alloc(0) }
      case 'host.state.put': {
        const previous = this.states.get(key)
        if ((previous?.version ?? null) !== params.expected_version) return fail('conflict')
        const version = (previous?.version ?? 0) + 1
        this.states.set(key, { value: structuredClone(params.value), version, schema_version: 1 })
        return { result: { version }, payload: Buffer.alloc(0) }
      }
      case 'host.data.accounts.list': {
        const query = JSON.parse(payload.toString())
        const remaining = this.accounts.filter(id => !query.cursor || id > query.cursor).sort()
        const ids = remaining.slice(0, query.limit)
        return { result: {}, payload: Buffer.from(JSON.stringify({
          schema_version: 1, next_cursor: remaining.length > ids.length ? ids.at(-1) : null,
          accounts: ids.map(account_id => ({
            account_id, provider_id: 'openai', name: this.accountNames.get(account_id) ?? `账号 ${account_id.slice(-3)}`,
            enabled: !this.disabledAccounts.has(account_id), email: null, group_ids: [], updated_at_ms: 1,
          })),
        })) }
      }
      default: throw new Error(`unexpected host method ${method}`)
    }
  }

  #fail(error) {
    this.#ready.reject(error)
    for (const call of this.#pending.values()) { clearTimeout(call.timeout); call.reject(error) }
    this.#pending.clear()
  }

  close() {
    this.#process.kill()
  }
}
