#include <QSqlDriver>
#include <QSqlError>

#ifdef __SQLITE3__
#include <sqlite3.h>
#endif // __SQLITE3__

#ifdef MIXXX_SQLITE3_FROM_QT
#include <dlfcn.h>

#include <atomic>
#include <mutex>
#endif // MIXXX_SQLITE3_FROM_QT

#include "util/db/dbconnection.h"

#include "util/db/sqllikewildcards.h"
#include "util/logger.h"
#include "util/assert.h"


// Originally from public domain code:
// http://www.archivum.info/qt-interest@trolltech.com/2008-12/00584/Re-%28Qt-interest%29-Qt-Sqlite-UserDefinedFunction.html

namespace mixxx {

namespace {

const mixxx::Logger kLogger("DbConnection");

QSqlDatabase createDatabase(
        const DbConnection::Params& params,
        const QString& connectionName) {
    kLogger.info()
        << "Available drivers for database connections:"
        << QSqlDatabase::drivers();

    QSqlDatabase database =
            QSqlDatabase::addDatabase(params.type, connectionName);
    database.setConnectOptions(params.connectOptions);
    database.setHostName(params.hostName);
    database.setDatabaseName(params.filePath);
    database.setUserName(params.userName);
    database.setPassword(params.password);
    return database;
}

QSqlDatabase cloneDatabase(
        const QSqlDatabase& database,
        const QString& connectionName) {
    DEBUG_ASSERT(!database.isOpen());
    return QSqlDatabase::cloneDatabase(database, connectionName);
}

void removeDatabase(
        QSqlDatabase* pDatabase) {
    DEBUG_ASSERT(pDatabase);
    DEBUG_ASSERT(!pDatabase->isOpen());
    // pDatabase must be the last reference to the implicitly shared
    // QSqlDatabase object
    QString connectionName = pDatabase->connectionName();
    // Drop the last reference before actually removing the database
    // to avoid the following warning:
    // "Warning [Main]: QSqlDatabasePrivate::removeDatabase: connection
    // '...' is still in use, all queries will cease to work."
    *pDatabase = QSqlDatabase();
    // After all references have been dropped we can safely remove the
    // connection. If still some of the afore mentioned warnings appear
    // in the log than a component is misbehaving and still holding an
    // invalid copy of the QSqlDatabase object that it shouldn't have!!
    QSqlDatabase::removeDatabase(connectionName);
}

void makeLatinLow(QChar* c, int count) {
    for (int i = 0; i < count; ++i) {
        if (c[i].decompositionTag() != QChar::NoDecomposition) {
            QString decomposition = c[i].decomposition();
            if (!decomposition.isEmpty() && !decomposition[0].isSpace()) {
                // here we remove the decoration from all characters.
                // We want "o" matching "ó" and all other variants but we
                // do not decompose decoration only characters like "˚" where
                // the base character is a space
                c[i] = decomposition.at(0);
            }
        }
        if (c[i].isUpper()) {
            c[i] = c[i].toLower();
        }
    }
}

// Compare two strings for equality where the first string is
// a "LIKE" expression. Return true (1) if they are the same and
// false (0) if they are different.
// This is the original sqlite3 icuLikeCompare rewritten for QChar
int likeCompareInner(
        const QChar* pattern, // LIKE pattern
        int patternSize,
        const QChar* string, // The string to compare against
        int stringSize,
        const QChar esc) { // The escape character
    int iPattern = 0; // Current index in pattern
    int iString = 0; // Current index in string

    bool prevEscape = false; // True if the previous character was uEsc

    while (iPattern < patternSize) {
        // Read (and consume) the next character from the input pattern.
        QChar uPattern = pattern[iPattern++];
        // There are now 4 possibilities:
        // 1. uPattern is an unescaped match-all character "%",
        // 2. uPattern is an unescaped match-one character "_",
        // 3. uPattern is an unescaped escape character, or
        // 4. uPattern is to be handled as an ordinary character

        if (!prevEscape && uPattern == kSqlLikeMatchAll) {
            // Case 1.
            QChar c;

            // Skip any kSqlLikeMatchAll or kSqlLikeMatchOne characters that follow a
            // kSqlLikeMatchAll. For each kSqlLikeMatchOne, skip one character in the
            // test string.

            if (iPattern >= patternSize) {
                // Tailing %
                return 1;
            }

            while ((c = pattern[iPattern]) == kSqlLikeMatchAll || c == kSqlLikeMatchOne) {
                if (c == kSqlLikeMatchOne) {
                    if (++iString == stringSize) {
                        return 0;
                    }
                }
                if (++iPattern == patternSize) {
                    // Two or more tailing %
                    return 1;
                }
            }

            while (iString < stringSize) {
                if (likeCompareInner(&pattern[iPattern], patternSize - iPattern,
                                &string[iString], stringSize - iString, esc)) {
                    return 1;
                }
                iString++;
            }
            return 0;
        } else if (!prevEscape && uPattern == kSqlLikeMatchOne) {
            // Case 2.
            if (++iString == stringSize) {
                return 0;
            }
        } else if (!prevEscape && uPattern == esc) {
            // Case 3.
            prevEscape = true;
        } else {
            // Case 4.
            if (iString == stringSize) {
                return 0;
            }
            QChar uString = string[iString++];
            if (uString != uPattern) {
                return 0;
            }
            prevEscape = false;
        }
    }
    return iString == stringSize;
}

#ifdef __SQLITE3__

namespace {

const QChar kSqlLikeEscapeDefault = '\0';

#ifdef MIXXX_SQLITE3_FROM_QT
// Qt's SQLite driver has its own copy of SQLite built in. A handle it opened
// must only be used with the functions of that copy, so they are looked up
// in the library that holds the driver instead of being linked.
struct QtSqliteApi {
    decltype(&::sqlite3_initialize) initialize = nullptr;
    decltype(&::sqlite3_create_collation) create_collation = nullptr;
    decltype(&::sqlite3_create_function) create_function = nullptr;
    decltype(&::sqlite3_exec) exec = nullptr;
    decltype(&::sqlite3_busy_timeout) busy_timeout = nullptr;
    decltype(&::sqlite3_value_text) value_text = nullptr;
    decltype(&::sqlite3_result_int64) result_int64 = nullptr;
};

QtSqliteApi s_qtSqlite;
std::atomic<bool> s_collationInstalled = false;

void loadQtSqlite(const QSqlDriver* pDriver) {
    // The driver object's vtable lies in the library that holds the driver
    Dl_info info{};
    if (!dladdr(*reinterpret_cast<void* const*>(pDriver), &info) ||
            !info.dli_fname) {
        return;
    }
    void* pLib = dlopen(info.dli_fname, RTLD_NOW | RTLD_NOLOAD);
    if (!pLib) {
        return;
    }
    QtSqliteApi api;
    const auto resolve = [pLib](auto* pFunc, const char* name) {
        *pFunc = reinterpret_cast<std::remove_pointer_t<decltype(pFunc)>>(
                dlsym(pLib, name));
        return *pFunc != nullptr;
    };
    const bool ok = resolve(&api.initialize, "sqlite3_initialize") &&
            resolve(&api.create_collation, "sqlite3_create_collation") &&
            resolve(&api.create_function, "sqlite3_create_function") &&
            resolve(&api.exec, "sqlite3_exec") &&
            resolve(&api.busy_timeout, "sqlite3_busy_timeout") &&
            resolve(&api.value_text, "sqlite3_value_text") &&
            resolve(&api.result_int64, "sqlite3_result_int64");
    dlclose(pLib); // drops only the reference taken above
    if (ok) {
        s_qtSqlite = api;
    }
}

bool resolveQtSqlite(const QSqlDriver* pDriver) {
    static std::once_flag s_once;
    std::call_once(s_once, [pDriver] { loadQtSqlite(pDriver); });
    return s_qtSqlite.result_int64 != nullptr;
}

#define sqlite3_initialize s_qtSqlite.initialize
#define sqlite3_create_collation s_qtSqlite.create_collation
#define sqlite3_create_function s_qtSqlite.create_function
#define sqlite3_exec s_qtSqlite.exec
#define sqlite3_busy_timeout s_qtSqlite.busy_timeout
#define sqlite3_value_text s_qtSqlite.value_text
#define sqlite3_result_int64 s_qtSqlite.result_int64
#endif // MIXXX_SQLITE3_FROM_QT

} // anonymous namespace

// The collating function callback is invoked with a copy of the pArg
// application data pointer and with two strings in the encoding specified
// by the eTextRep argument.
// The collating function must return an integer that is negative, zero,
// or positive if the first string is less than, equal to, or greater
// than the second, respectively.
int sqliteStringCompareUTF16(void* pArg,
                             int len1, const void* data1,
                             int len2, const void* data2) {
    const auto* pCollator = static_cast<mixxx::StringCollator*>(pArg);
    // Construct a QString without copy
    QString string1 = QString::fromRawData(static_cast<const QChar*>(data1),
                                           len1 / sizeof(QChar));
    QString string2 = QString::fromRawData(static_cast<const QChar*>(data2),
                                           len2 / sizeof(QChar));
    return pCollator->compare(string1, string2);
}

const char kLexicographicalCollationFunc[] = "mixxxLexicographicalCollationFunc";

/// The Latin letters of a lower-case Greek letter without accent, or nullptr
const char* greekToLatin(char16_t c) {
    switch (c) {
    case u'α':
        return "a";
    case u'β':
        return "v";
    case u'γ':
        return "g";
    case u'δ':
        return "d";
    case u'ε':
        return "e";
    case u'ζ':
        return "z";
    case u'η':
        return "i";
    case u'θ':
        return "8";
    case u'ι':
    case u'ϊ':
        return "i";
    case u'κ':
        return "k";
    case u'λ':
        return "l";
    case u'μ':
        return "m";
    case u'ν':
        return "n";
    case u'ξ':
        return "x";
    case u'ο':
        return "o";
    case u'π':
        return "p";
    case u'ρ':
        return "r";
    case u'σ':
    case u'ς':
        return "s";
    case u'τ':
        return "t";
    case u'υ':
    case u'ϋ':
        return "i";
    case u'φ':
        return "f";
    case u'χ':
        return "x";
    case u'ψ':
        return "ps";
    case u'ω':
        return "o";
    default:
        return nullptr;
    }
}

/// The sound of two letters that Greeklish writes in different ways, or 0
char16_t latinPair(char16_t c, char16_t next) {
    switch (c) {
    case u'm':
        return next == u'p' ? u'v' : 0; // μπ, like b
    case u'n':
        return next == u't' ? u'd' : 0; // ντ
    case u'g':
        return next == u'k' || next == u'g' ? u'g' : 0;
    case u'o': // ου as ou/oy/u, οι as oi/i
        return next == u'u' || next == u'y' ? u'u' : next == u'i' ? u'i'
                                                                  : 0;
    case u'a':
        return next == u'i' ? u'e' : 0;
    case u'e':
        return next == u'i' ? u'i' : 0;
    case u't':
        return next == u'h' ? u'8' : 0;
    case u'c':
        return next == u'h' ? u'x' : 0;
    case u'p':
        return next == u'h' ? u'f' : 0;
    case u'k':
        return next == u's' ? u'x' : 0;
    default:
        return 0;
    }
}

/// A key that Greek and Greeklish spellings of a word share, with or without
/// accents, e.g. "Λιόλιου", "λιολιου" and "lioliou" all give "lioliu": Greek
/// letters as Latin ones, then the sounds Greeklish writes in different ways
/// the same way (ou/u, ei/oi/i/y/h/η, ai/e, w/o, 8/th, ks/x, ch/x, b/v/mp,
/// eu/ef/ev), and no double letters. Wildcards and escaped characters stay.
/// One pass with no lookups beyond a switch: it runs for every row searched.
void greeklishKey(const QString& text, QChar esc, QString* pKey) {
    thread_local QString latin;
    latin.resize(0);
    latin.reserve(text.size() + 8);
    const int size = static_cast<int>(text.size());
    for (int i = 0; i < size; ++i) {
        QChar c = text[i];
        if (!esc.isNull() && c == esc && i + 1 < size) {
            latin.append(c);
            latin.append(text[++i]);
            continue;
        }
        if (c.unicode() < 0x80) {
            latin.append(c.toLower());
            continue;
        }
        makeLatinLow(&c, 1);
        // ου is u, αυ/ευ are av/ev, whatever else υ is
        if ((c == u'ο' || c == u'α' || c == u'ε') && i + 1 < size) {
            QChar next = text[i + 1];
            makeLatinLow(&next, 1);
            if (next == u'υ') {
                if (c == u'ο') {
                    latin.append(u'u');
                } else {
                    latin.append(c == u'α' ? u'a' : u'e');
                    latin.append(u'v');
                }
                ++i;
                continue;
            }
        }
        if (const char* pLatin = greekToLatin(c.unicode())) {
            latin.append(QLatin1String(pLatin));
        } else {
            latin.append(c);
        }
    }
    QString& key = *pKey;
    key.resize(0);
    key.reserve(latin.size());
    const int latinSize = static_cast<int>(latin.size());
    for (int i = 0; i < latinSize; ++i) {
        char16_t c = latin[i].unicode();
        if (!esc.isNull() && c == esc.unicode() && i + 1 < latinSize) {
            key.append(latin[i]);
            key.append(latin[++i]);
            continue;
        }
        const char16_t next = i + 1 < latinSize ? latin[i + 1].unicode() : 0;
        if ((c == u'e' || c == u'a') && (next == u'u' || next == u'f')) {
            key.append(QChar(c)); // eu/ef/ev, au/af/av
            c = u'v';
            ++i;
        } else if (const char16_t pair = latinPair(c, next)) {
            c = pair;
            ++i;
        } else if (c == u'y' || c == u'h') {
            c = u'i';
        } else if (c == u'w') {
            c = u'o';
        } else if (c == u'b') {
            c = u'v';
        }
        if (key.isEmpty() || key.back() != QChar(c) || c == u'%') {
            key.append(QChar(c));
        }
    }
}

// This implements the like() SQL function. This is used by the LIKE operator.
// The SQL statement 'A LIKE B' is implemented as 'like(B, A)', and if there is
// an escape character, say E, it is implemented as 'like(B, A, E)'
//static
void sqliteLikeUtf8(sqlite3_context* context,
        int aArgc,
        sqlite3_value** aArgv) {
    VERIFY_OR_DEBUG_ASSERT(aArgc == 2 || aArgc == 3) {
        return;
    }

    const char* b = reinterpret_cast<const char*>(
            sqlite3_value_text(aArgv[0]));
    const char* a = reinterpret_cast<const char*>(
            sqlite3_value_text(aArgv[1]));

    if (!a || !b) {
        return;
    }

    QString stringB = QString::fromUtf8(b); // Like String
    QString stringA = QString::fromUtf8(a);

    QChar esc = kSqlLikeEscapeDefault;
    if (aArgc == 3) {
        const char* e = reinterpret_cast<const char*>(
                sqlite3_value_text(aArgv[2]));
        if (e) {
            QString stringE = QString::fromUtf8(e);
            if (!stringE.isEmpty()) {
                esc = stringE.data()[0];
            }
        }
    }

    int ret = DbConnection::likeCompareLatinLow(&stringB, &stringA, esc);
    if (!ret) {
        // Greek titles found by Greeklish and the other way round, and
        // Greeklish written one way by Greeklish written another (w/o, 8/th)
        // The pattern is the same for every row of a search
        thread_local QString lastPattern;
        thread_local QChar lastEsc;
        thread_local QString pattern;
        if (stringB != lastPattern || esc != lastEsc) {
            lastPattern = stringB;
            lastEsc = esc;
            greeklishKey(stringB, esc, &pattern);
        }
        thread_local QString string;
        greeklishKey(stringA, QChar(), &string);
        ret = likeCompareInner(pattern.data(),
                pattern.length(),
                string.data(),
                string.length(),
                esc);
    }
    sqlite3_result_int64(context, ret);
    return;
}

#endif // __SQLITE3__

bool initDatabase(const QSqlDatabase& database, mixxx::StringCollator* pCollator) {
    DEBUG_ASSERT(database.isOpen());
#ifdef __SQLITE3__
    QVariant v = database.driver()->handle();
    VERIFY_OR_DEBUG_ASSERT(v.isValid()) {
        kLogger.warning() << "Driver handle is invalid";
        return false; // abort
    }
    if (strcmp(v.typeName(), "sqlite3*") != 0) {
        kLogger.warning()
                << "Unsupported database driver:"
               << v.typeName();
        return false; // abort
    }
#ifdef MIXXX_SQLITE3_FROM_QT
    if (!resolveQtSqlite(database.driver())) {
        kLogger.warning() << "SQLite functions of the Qt driver not found:"
                          << "plain LIKE search and default journal";
        return true;
    }
#endif // MIXXX_SQLITE3_FROM_QT
    // Ensure initialization. sqlite recommends doing this before using it
    // and might become required in future versions
    int rc = sqlite3_initialize();
    VERIFY_OR_DEBUG_ASSERT(rc == SQLITE_OK) {
        kLogger.warning()
            << "sqlite3_initialize failed with the code: "
            << rc;
    }

    // v.data() returns a pointer to the handle
    sqlite3* handle = *static_cast<sqlite3**>(v.data());
    VERIFY_OR_DEBUG_ASSERT(handle != nullptr) {
        kLogger.warning()
                << "SQLite3 handle is invalid";
        return false; // abort
    }

    int result = sqlite3_create_collation(
                    handle,
                    kLexicographicalCollationFunc,
                    SQLITE_UTF16,
                    pCollator,
                    sqliteStringCompareUTF16);
    VERIFY_OR_DEBUG_ASSERT(result == SQLITE_OK) {
        kLogger.warning()
                << "Failed to install locale-aware lexicographical collation function for SQLite3:"
                << result;
    }
#ifdef MIXXX_SQLITE3_FROM_QT
    if (result == SQLITE_OK) {
        s_collationInstalled = true;
    }
#endif // MIXXX_SQLITE3_FROM_QT

    result = sqlite3_create_function(
            handle,
            "like",
            2,
            SQLITE_UTF8 | SQLITE_DETERMINISTIC,
            nullptr,
            sqliteLikeUtf8,
            nullptr,
            nullptr);
    VERIFY_OR_DEBUG_ASSERT(result == SQLITE_OK) {
        kLogger.warning()
                << "Failed to install custom 2-arg LIKE function for SQLite3:"
                << result;
    }

    result = sqlite3_create_function(
            handle,
            "like",
            3, // 3rd arg = ESCAPE
            SQLITE_UTF8 | SQLITE_DETERMINISTIC,
            nullptr,
            sqliteLikeUtf8,
            nullptr,
            nullptr);
    VERIFY_OR_DEBUG_ASSERT(result == SQLITE_OK) {
        kLogger.warning()
                << "Failed to install custom 3-arg LIKE function for SQLite3:"
                << result;
    }

    // Enable WAL (Write-Ahead Logging) journal mode and a busy timeout so
    // that concurrent database access from multiple threads (e.g. the
    // library scanner running in its own thread while the GUI thread is
    // also writing) does not fail with SQLITE_BUSY:
    // - WAL allows concurrent readers while a single writer is active.
    //   This is a database-level, persistent setting: setting it once on
    //   any connection persists it for all subsequent connections. The
    //   PRAGMA is silently ignored for in-memory databases (as used in
    //   tests).
    // - The busy timeout makes SQLite wait and retry instead of failing
    //   immediately when two connections compete for the write lock.
    result = sqlite3_exec(handle, "PRAGMA journal_mode=WAL", nullptr, nullptr, nullptr);
    if (result != SQLITE_OK) {
        kLogger.warning()
                << "Failed to enable WAL journal mode for SQLite3:"
                << result;
    }

    // WAL is on: NORMAL keeps the database consistent on a crash and skips
    // an fsync per transaction (the scanner commits per track). The page
    // cache (per connection, 16 MB) holds the library table's hot pages,
    // sorts and temp B-trees stay in memory, and ANALYZE samples instead
    // of reading every row.
    for (const char* pragma : {"PRAGMA synchronous=NORMAL",
                 "PRAGMA cache_size=-16384",
                 "PRAGMA temp_store=MEMORY",
                 "PRAGMA analysis_limit=1000"}) {
        result = sqlite3_exec(handle, pragma, nullptr, nullptr, nullptr);
        if (result != SQLITE_OK) {
            kLogger.warning() << "Failed" << pragma << result;
        }
    }

    result = sqlite3_busy_timeout(handle, 30000);
    if (result != SQLITE_OK) {
        kLogger.warning()
                << "Failed to set busy timeout for SQLite3:"
                << result;
    }
#else
    Q_UNUSED(database);
    Q_UNUSED(pCollator);
#endif // __SQLITE3__
    return true;
}

} // anonymous namespace

DbConnection::DbConnection(
        const Params& params,
        const QString& connectionName)
    : m_sqlDatabase(createDatabase(params, connectionName)) {
}

DbConnection::DbConnection(
        const DbConnection& prototype,
        const QString& connectionName)
    : m_sqlDatabase(cloneDatabase(prototype.m_sqlDatabase, connectionName)) {
}

DbConnection::~DbConnection() {
    close();
    removeDatabase(&m_sqlDatabase);
}

bool DbConnection::open() {
    if (kLogger.debugEnabled()) {
        kLogger.debug()
                << "Opening database connection"
                << *this;
    }
    if (!m_sqlDatabase.open()) {
        kLogger.warning()
                << "Failed to open database connection"
                << *this
                << m_sqlDatabase.lastError();
        return false; // abort
    }
    if (!initDatabase(m_sqlDatabase, &m_collator)) {
        kLogger.warning()
                << "Failed to initialize database connection"
                << *this;
        m_sqlDatabase.close();
        return false; // abort
    }
    return true;
}

void DbConnection::close() {
    if (m_sqlDatabase.isOpen()) {
        // There should never be an outstanding transaction when this code is
        // called. If there is, it means we probably aren't committing a
        // transaction somewhere that should be.
        VERIFY_OR_DEBUG_ASSERT(!m_sqlDatabase.rollback()) {
            kLogger.warning()
                << "Rolled back open transaction before closing database connection:"
                << *this;
        }
        if (kLogger.debugEnabled()) {
            kLogger.debug()
                    << "Closing database connection:"
                    << *this;
        }
        m_sqlDatabase.close();
    }
}

//static
QString DbConnection::collateLexicographically(const QString& orderByQuery) {
#ifdef __SQLITE3__
#ifdef MIXXX_SQLITE3_FROM_QT
    if (!s_collationInstalled) {
        return orderByQuery;
    }
#endif // MIXXX_SQLITE3_FROM_QT
    return orderByQuery + QStringLiteral(" COLLATE ") + kLexicographicalCollationFunc;
#else
        return orderByQuery;
#endif //  __SQLITE3__
}

//static
int DbConnection::likeCompareLatinLow(
        QString* pattern,
        QString* string,
        QChar esc) {
    makeLatinLow(pattern->data(), pattern->length());
    makeLatinLow(string->data(), string->length());
    return likeCompareInner(
            pattern->data(), pattern->length(),
            string->data(), string->length(),
            esc);
}

//static
void DbConnection::makeStringLatinLow(QString* string) {
    makeLatinLow(string->data(), string->length());
}

QDebug operator<<(QDebug debug, const DbConnection& connection) {
    return debug
            << connection.name()
            << connection.m_sqlDatabase;
}

} // namespace mixxx
