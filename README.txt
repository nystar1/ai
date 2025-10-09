ai.hackclub.com

An experimental service providing unlimited /chat/completions for free, for teens in Hack Club.
No API key needed.

Example usage:

curl -X POST https://ai.hackclub.com/chat/completions \
    -H "Content-Type: application/json" \
    -d '{
        "messages": [{"role": "user", "content": "Tell me a joke!"}]
    }'

This project is dual licensed with Apache2 and MIT. 
Visit https://ai.hackclub.com for more details about the service.