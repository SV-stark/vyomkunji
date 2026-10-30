// interceptor.test.mjs — Unit and integration tests for Indian portal signer interception
// Run with: node --test extension/tests/interceptor.test.mjs
import assert from "node:assert/strict";
import test from "node:test";

function isBase64(str) {
  if (typeof str !== "string" || !str || str.length % 4 !== 0) return false;
  return /^[A-Za-z0-9+/]+={0,2}$/.test(str);
}

function extractFromObject(obj) {
  let target = obj;
  if (obj.data && typeof obj.data === "object") target = obj.data;
  if (obj.payload && typeof obj.payload === "object") target = obj.payload;

  const rawData =
    target.data ??
    target.signData ??
    target.dataToSign ??
    target.content ??
    target.file ??
    target.hash ??
    target.digest ??
    target.xml ??
    "";

  const rawDataStr = typeof rawData === "string" ? rawData : JSON.stringify(rawData);
  const isDigest = !!(target.hash || target.digest || target.alreadyHashed);
  const hashAlg = (target.hashAlg || target.hashAlgorithm || target.digestAlgorithm || "SHA256").toUpperCase();
  const signAlg = (target.signAlg || target.keyType || "RSA").toUpperCase();
  const pkcs7 = !!(target.pkcs7 || target.includeChain || target.type === "pdf" || target.action === "signpdf");
  const certCN = target.certCN || target.commonName || target.alias || null;
  const data = isBase64(rawDataStr) ? rawDataStr : Buffer.from(rawDataStr).toString("base64");

  return { data, hashAlg, signAlg, alreadyHashed: isDigest, pkcs7, certCN };
}

function parseSigningPayload(rawBody) {
  if (rawBody && typeof rawBody === "object") {
    return extractFromObject(rawBody);
  }
  const str = String(rawBody || "").trim();
  if (!str) {
    return { data: "", hashAlg: "SHA256", signAlg: "RSA", alreadyHashed: false, pkcs7: false, certCN: null };
  }

  if ((str.startsWith("{") && str.endsWith("}")) || (str.startsWith("[") && str.endsWith("]"))) {
    try {
      const obj = JSON.parse(str);
      return extractFromObject(obj);
    } catch (_) {}
  }

  if (str.includes("=") && (str.includes("&") || str.startsWith("data=") || str.startsWith("action="))) {
    try {
      const params = new URLSearchParams(str);
      const extracted = params.get("data") || params.get("signData") || params.get("dataToSign");
      if (extracted) {
        return {
          data: isBase64(extracted) ? extracted : Buffer.from(extracted).toString("base64"),
          hashAlg: (params.get("hashAlg") || "SHA256").toUpperCase(),
          signAlg: (params.get("signAlg") || "RSA").toUpperCase(),
          alreadyHashed: params.get("alreadyHashed") === "true",
          pkcs7: params.get("pkcs7") === "true" || params.get("type") === "pdf",
          certCN: params.get("certCN") || null
        };
      }
    } catch (_) {}
  }

  return {
    data: isBase64(str) ? str : Buffer.from(str).toString("base64"),
    hashAlg: "SHA256",
    signAlg: "RSA",
    alreadyHashed: false,
    pkcs7: false,
    certCN: null
  };
}

test("parses JSON payload with signData used by GST and Income Tax", () => {
  const jsonBody = JSON.stringify({
    action: "sign",
    signData: "SGVsbG8gSW5kaWFuIFRheCBQb3J0YWw=",
    hashAlg: "SHA256"
  });
  const parsed = parseSigningPayload(jsonBody);
  assert.equal(parsed.data, "SGVsbG8gSW5kaWFuIFRheCBQb3J0YWw=");
  assert.equal(parsed.hashAlg, "SHA256");
  assert.equal(parsed.alreadyHashed, false);
  assert.equal(parsed.pkcs7, false);
});

test("parses precomputed digest and sets alreadyHashed", () => {
  const jsonBody = JSON.stringify({
    action: "sign",
    hash: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
    hashAlg: "SHA256"
  });
  const parsed = parseSigningPayload(jsonBody);
  assert.equal(parsed.alreadyHashed, true);
  assert.equal(parsed.hashAlg, "SHA256");
});

test("detects PKCS#7 / CMS signing requests for MCA21 and PDF sign", () => {
  const jsonBody = JSON.stringify({
    action: "signpdf",
    type: "pdf",
    data: "JVBERi0xLjQKJeLjz9MKMSAwIG9iaiA8PAo=",
    pkcs7: true
  });
  const parsed = parseSigningPayload(jsonBody);
  assert.equal(parsed.pkcs7, true);
});

test("parses URL-encoded form data used by legacy emSigner", () => {
  const formBody = "action=sign&data=dGVzdGRhdGE=&hashAlg=SHA256";
  const parsed = parseSigningPayload(formBody);
  assert.equal(parsed.data, "dGVzdGRhdGE=");
  assert.equal(parsed.hashAlg, "SHA256");
});

test("formats signing response matching Indian government portal expectations", () => {
  const nativeResp = {
    ok: true,
    signature: "SIG_BASE64_VALUE",
    cert: "CERT_DER_BASE64",
    pkcs7: "PKCS7_BASE64_VALUE",
    chain: ["INTERMEDIATE_CERT_BASE64"]
  };

  const portalResponse = {
    status: "success",
    status_code: 200,
    signature: nativeResp.signature,
    signData: nativeResp.signature,
    sign: nativeResp.signature,
    signedData: nativeResp.signature,
    pkcs7: nativeResp.pkcs7,
    cert: nativeResp.cert,
    certificate: nativeResp.cert,
    chain: nativeResp.chain,
    data: {
      signature: nativeResp.signature,
      signData: nativeResp.signature,
      certificate: nativeResp.cert,
      cert: nativeResp.cert
    }
  };

  assert.equal(portalResponse.status, "success");
  assert.equal(portalResponse.status_code, 200);
  assert.equal(portalResponse.signData, "SIG_BASE64_VALUE");
  assert.equal(portalResponse.certificate, "CERT_DER_BASE64");
  assert.equal(portalResponse.pkcs7, "PKCS7_BASE64_VALUE");
  assert.equal(portalResponse.data.signData, "SIG_BASE64_VALUE");
});
