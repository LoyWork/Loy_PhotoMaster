// Build-time provisioning only. Installed Picturama never downloads models or photos.
import { createHash } from 'node:crypto'
import { createReadStream, createWriteStream, existsSync } from 'node:fs'
import { copyFile, mkdir, mkdtemp, readFile, rename, rm, writeFile } from 'node:fs/promises'
import { pipeline } from 'node:stream/promises'
import { Readable } from 'node:stream'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { execFileSync } from 'node:child_process'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..')
const assets = join(root, 'src-tauri/models')
const modelHash = 'f22797eabf810a75e41de68d378541ebea372122b25c4ce3ef25ff618250c20a'
const runtimeHash = '579b636403983254346a5c1d80bd28f1519cd1e284cd204f8d4ff41f8d711559'
const archiveHash = '174c616efc0271194488642a72f1a514e01487da4dfe84c49296d66e40ebe0da'
const revision = '8b1f705a3a7f6f062f6bdd21986c1583d3ef105d'

async function hash(path) {
    if (!existsSync(path)) return null
    const digest = createHash('sha256')
    for await (const chunk of createReadStream(path)) digest.update(chunk)
    return digest.digest('hex')
}

async function download(url, path, expectedHash) {
    if (expectedHash && await hash(path) === expectedHash) return
    console.log(`Downloading ${url}`)
    const response = await fetch(url, { signal: AbortSignal.timeout(300_000) })
    if (!response.ok || !response.body) throw new Error(`Download failed: ${response.status} ${url}`)
    const partial = `${path}.part`
    try {
        await pipeline(Readable.fromWeb(response.body), createWriteStream(partial))
        if (expectedHash && await hash(partial) !== expectedHash) throw new Error(`Checksum mismatch: ${path}`)
        await rename(partial, path)
    } finally { await rm(partial, { force: true }) }
}

if (process.platform !== 'win32' || process.arch !== 'x64') {
    console.log('Bundled DINOv2 CPU runtime currently targets Windows x64. Quick scanning remains available.')
} else {
    await mkdir(assets, { recursive: true })
    await download(`https://huggingface.co/onnx-community/dinov2-small/resolve/${revision}/onnx/model.onnx`, join(assets, 'dinov2-small.onnx'), modelHash)
    if (await hash(join(assets, 'onnxruntime.dll')) !== runtimeHash ||
        !existsSync(join(assets, 'ONNXRuntime-LICENSE.txt')) || !existsSync(join(assets, 'ONNXRuntime-ThirdPartyNotices.txt'))) {
        const archive = join(assets, 'runtime.zip')
        await download('https://github.com/microsoft/onnxruntime/releases/download/v1.22.0/onnxruntime-win-x64-1.22.0.zip', archive, archiveHash)
        const scratch = await mkdtemp(join(tmpdir(), 'picturama-model-assets-'))
        try {
            const prefix = 'onnxruntime-win-x64-1.22.0'
            execFileSync('tar', ['-xf', archive, '-C', scratch,
                `${prefix}/lib/onnxruntime.dll`, `${prefix}/LICENSE`, `${prefix}/ThirdPartyNotices.txt`])
            const runtime = join(scratch, prefix, 'lib/onnxruntime.dll')
            if (await hash(runtime) !== runtimeHash) throw new Error('Runtime DLL checksum mismatch')
            await copyFile(runtime, join(assets, 'onnxruntime.dll'))
            await copyFile(join(scratch, prefix, 'LICENSE'), join(assets, 'ONNXRuntime-LICENSE.txt'))
            await copyFile(join(scratch, prefix, 'ThirdPartyNotices.txt'), join(assets, 'ONNXRuntime-ThirdPartyNotices.txt'))
        } finally {
            if (resolve(scratch).startsWith(resolve(tmpdir()) + '\\') && dirname(scratch) === resolve(tmpdir())) await rm(scratch, { recursive: true, force: true })
        }
        await rm(archive, { force: true })
    }
    if (!existsSync(join(assets, 'DINOv2-LICENSE.txt'))) {
        await download('https://raw.githubusercontent.com/facebookresearch/dinov2/main/LICENSE', join(assets, 'DINOv2-LICENSE.txt'))
    }
    await writeFile(join(assets, 'provenance.json'), JSON.stringify({
        model: 'facebook/dinov2-small', export: 'onnx-community/dinov2-small', revision,
        modelSha256: modelHash, runtimeVersion: '1.22.0', runtimeSha256: runtimeHash,
    }, null, 2) + '\n')
    // Check that the license was downloaded as text rather than an error page.
    if (!(await readFile(join(assets, 'DINOv2-LICENSE.txt'), 'utf8')).includes('Apache License')) throw new Error('Invalid model license')
    console.log('DINOv2 scene matching assets verified and ready (local CPU inference).')
}
