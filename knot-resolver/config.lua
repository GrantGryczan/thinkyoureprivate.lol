local http_request = require("http.request")
local http_util = require("http.util")

local TIMEOUT_SECONDS = 3

local ipv4_request = http_request.new_from_uri("https://ipv4.icanhazip.com/")
local ipv6_request = http_request.new_from_uri("https://ipv6.icanhazip.com/")
local ipv4_headers, ipv4_stream = ipv4_request:go(TIMEOUT_SECONDS)
local ipv6_headers, ipv6_stream = ipv6_request:go(TIMEOUT_SECONDS)

local answer = {}

if ipv4_headers and ipv4_headers:get(":status") == "200" then
    local ipv4_addr = assert(ipv4_stream:get_body_as_string()):gsub("%s+", "")
    answer[kres.type.A] = { rdata = kres.str2ip(ipv4_addr), ttl = 30 }
end

if ipv6_headers and ipv6_headers:get(":status") == "200" then
    local ipv6_addr = assert(ipv6_stream:get_body_as_string()):gsub("%s+", "")
    answer[kres.type.AAAA] = { rdata = kres.str2ip(ipv6_addr), ttl = 30 }
end

answer = policy.ANSWER(answer)
policy.add(policy.suffix(answer, policy.todnames({ "mydomain.here" })))

local function handle_query(state, req)
    local query = req:current()
    local sname = kres.dname2str(query.sname)
    local subdomain = sname:match("^(.+)%.mydomain%.here%.$")

    if subdomain == nil then
        return nil
    end

    local uri_query = http_util.dict_to_query({
        subdomain = subdomain,
    })

    local ip_addr = tostring(req.qsource.addr):gsub("#.*", "")

    local uri = "http://web/dns-query?" .. uri_query
    local request = http_request.new_from_uri(uri)
    request.headers:upsert(":method", "POST")
    request:set_body(ip_addr)
    local headers, stream = assert(request:go(TIMEOUT_SECONDS))

    local status = headers:get(":status")
    if status ~= "200" then
        local body = assert(stream:get_body_as_string())
        error(status .. " " .. body)
    end

    return nil
end

policy.add(policy.suffix(handle_query, policy.todnames({ "mydomain.here" })))
