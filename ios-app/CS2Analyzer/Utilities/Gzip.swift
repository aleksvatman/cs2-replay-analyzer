import Compression
import Foundation

enum GzipError: Error, LocalizedError {
    case notGzip
    case truncated
    case decompressionFailed

    var errorDescription: String? {
        switch self {
        case .notGzip: return "Не gzip (нет сигнатуры 1f 8b)"
        case .truncated: return "Файл gzip оборван"
        case .decompressionFailed: return "Не удалось распаковать gzip"
        }
    }
}

extension Data {
    /// Распаковывает gzip (`match.json.gz` с сервера или локального разбора). Сжатие всегда
    /// deflate (CM=8): заголовок и трейлер (CRC32 + ISIZE) снимаются вручную, тело
    /// распаковывается как raw deflate через `Compression` (`COMPRESSION_ZLIB` в этом API —
    /// это алгоритм deflate без zlib-обёртки, несмотря на название).
    func gunzipped() throws -> Data {
        guard count >= 18 else { throw GzipError.truncated }
        let bytes = [UInt8](self)
        guard bytes[0] == 0x1f, bytes[1] == 0x8b else { throw GzipError.notGzip }
        guard bytes[2] == 8 else { throw GzipError.decompressionFailed } // CM must be deflate

        let flags = bytes[3]
        var offset = 10
        func requireRoom(_ extra: Int) throws {
            guard offset + extra <= bytes.count else { throw GzipError.truncated }
        }
        if flags & 0x04 != 0 { // FEXTRA
            try requireRoom(2)
            let xlen = Int(bytes[offset]) | (Int(bytes[offset + 1]) << 8)
            offset += 2
            try requireRoom(xlen)
            offset += xlen
        }
        if flags & 0x08 != 0 { // FNAME
            while offset < bytes.count, bytes[offset] != 0 { offset += 1 }
            offset += 1
        }
        if flags & 0x10 != 0 { // FCOMMENT
            while offset < bytes.count, bytes[offset] != 0 { offset += 1 }
            offset += 1
        }
        if flags & 0x02 != 0 { // FHCRC
            try requireRoom(2)
            offset += 2
        }
        guard offset + 8 <= bytes.count else { throw GzipError.truncated }

        let isizeBytes = bytes.suffix(4)
        let isize = isizeBytes.reversed().reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
        let deflateStart = offset
        let deflateEnd = bytes.count - 8
        guard deflateEnd > deflateStart else { throw GzipError.truncated }

        // ISIZE это размер по модулю 2^32; 0 означает «неизвестно» (пустой файл или >= 4 ГБ).
        // Такого размера не бывает в наших данных, поэтому растим буфер при недостаче.
        var capacity = isize > 0 ? Int(isize) : Swift.max(deflateEnd - deflateStart, 1) * 4
        while true {
            var destination = [UInt8](repeating: 0, count: capacity)
            let produced = destination.withUnsafeMutableBytes { dst -> Int in
                bytes.withUnsafeBytes { src -> Int in
                    let srcStart = src.baseAddress!.advanced(by: deflateStart)
                    return compression_decode_buffer(
                        dst.bindMemory(to: UInt8.self).baseAddress!, capacity,
                        srcStart.assumingMemoryBound(to: UInt8.self), deflateEnd - deflateStart,
                        nil, COMPRESSION_ZLIB
                    )
                }
            }
            if produced == 0, capacity < (deflateEnd - deflateStart) * 64 {
                // Буфер не подошёл (оценка ISIZE была не той) — пробуем ещё раз побольше.
                capacity *= 2
                continue
            }
            guard produced > 0 else { throw GzipError.decompressionFailed }
            return Data(destination.prefix(produced))
        }
    }
}
