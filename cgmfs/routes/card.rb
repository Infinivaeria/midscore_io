class CGMFS
  hash_branch 'card' do |r|
    r.on do
      r.get do
        response['Content-Type'] = 'image/jpeg'
        File.binread('/root/midscore_io/public/card_banner2.jpg')
      end
    end
  end
end
