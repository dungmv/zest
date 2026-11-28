# Đặc tả Kỹ thuật: Đồng bộ Git & Cộng tác Nhóm không phụ thuộc Cloud (Git Sync & Collaboration)

## 1. Triết lý: Tại sao Local-First & Git-Native là Tương lai?
- **Khủng hoảng Cloud Lock-in của Postman/Insomnia:**
  - Postman bắt buộc lưu trữ dữ liệu API lên server của Postman. Nhiều công ty tài chính, y tế, an ninh mạng **nghiêm cấm** nhân viên sử dụng Postman vì lo ngại rò rỉ thông tin nội bộ (API Keys, Production Endpoints, PII Data).
  - Insomnia v8 yêu cầu login khiến nhiều tổ chức phải đóng băng phiên bản v7 cũ.
- **Giải pháp của Zest:**
  - Sử dụng chính hạ tầng Git sẵn có của tổ chức (GitHub, GitLab, Bitbucket, hoặc Server nội bộ).
  - Workspace nằm trên hệ thống tệp cục bộ và có thể được quản lý bằng Git.
  - Mỗi request là một tệp YAML, giúp `git diff` hiển thị thay đổi theo dòng để review pull request.

---

## 2. Cấu trúc Thư mục Git trong Workspace

```
my-api-project/
├── .git/                       # Metadata Git của bản clone
├── .gitignore                  # Bỏ qua .local/ và *.loc
├── workspace.yaml              # Metadata của workspace
├── environments/
│   └── staging.yaml            # Biến dùng chung và khai báo biến bí mật
├── collections/
│   └── auth/
│       ├── _folder.yaml        # Metadata thư mục
│       └── login.yaml          # Request POST /login
└── .local/
    └── secrets/
        └── staging.yaml        # Giá trị bí mật trên máy hiện tại
```

### 2.1 Biến dùng chung và giá trị bí mật

- Tệp `environments/staging.yaml` có thể được commit để chia sẻ tên biến, giá trị thông thường và cờ `secret: true` của biến bí mật.
- Giá trị bí mật được lưu trong `.local/secrets/`. Sau khi mở bản clone, người dùng nhập giá trị bí mật cần thiết trên máy của mình.

---

## 3. Tính năng Git Tích Hợp trong Giao diện UI

Thay vì bắt người dùng phải mở cửa sổ Terminal rời để gõ lệnh `git`, Zest tích hợp một Git Drawer chuyên dụng:

1. **Thanh trạng thái Branch & Dirty Indicator:**
   - Góc trái titlebar và footer hiển thị branch hiện tại (ví dụ: `⎇ feature/auth-v2`).
   - Có chấm cam nhỏ nếu có thay đổi chưa commit (`3 modified, 1 untracked`).
2. **Visual Request Diff Viewer:**
   - Khi nhấp vào một request có thay đổi, hiển thị giao diện so sánh trước/sau (Side-by-side Diff):
     - Headers nào bị thêm/xóa.
     - Body JSON thay đổi ở trường nào (được tô màu xanh lá cho trường thêm mới, đỏ cho trường bị xóa).
3. **Thao tác 1-Click Git Operations:**
   - **Branch Picker:** Tạo branch mới, chuyển branch nhanh.
   - **Commit Dialog:** Nhập commit message và bấm **Commit Changes**.
   - **Sync:** Nút **Pull** (cập nhật từ remote) và **Push** (đẩy lên remote).
   - **Conflict Resolver:** Giao diện trực quan khi xảy ra merge conflict, cho phép chọn `Use Local`, `Use Remote`, hoặc `Merge Both`.

---

## 4. Mở workspace từ Git repository

Người dùng có thể mở workspace Zest đã được chia sẻ qua Git bằng cách cung cấp URL và chọn nơi lưu bản clone trên máy. Lệnh **Open Workspace from Git…** nằm cạnh **Open Workspace…** trong menu workspace. Người dùng có sẵn bản clone có thể chọn thư mục qua **Open Workspace…**.

### 4.1 Luồng mở workspace

1. Nhập URL clone HTTPS hoặc SSH và chọn một thư mục cha có sẵn. Zest lấy tên repo làm tên thư mục con. Ví dụ, `git@example.com:team/apidocs.git` được clone vào `apidocs` trong thư mục đã chọn. Thư mục cha có thể chứa dữ liệu. Nếu thư mục con đã chứa bản clone, hộp thoại hướng người dùng đến **Open Workspace…**.
2. Chọn **Clone and Open**. Zest clone vào thư mục tạm cạnh đích, checkout nhánh mặc định của remote, hiển thị trạng thái đang clone và cho phép hủy. Git sử dụng credential helper hoặc SSH agent đã cấu hình trên máy. URL chứa mật khẩu hoặc token nhúng bị từ chối và thông báo lỗi phải loại bỏ thông tin xác thực.
3. Sau khi clone, Zest kiểm tra `workspace.yaml` tại gốc bản checkout và đọc tệp bằng schema hiện hành. Yêu cầu có tệp này áp dụng cả với workspace chưa có request. Nếu repo thiếu tệp hoặc metadata không hợp lệ, hộp thoại báo lỗi để người dùng sửa nguồn hoặc chọn repo khác. Luồng này chỉ mở workspace đã có sẵn.
4. Sau khi kiểm tra metadata, Zest kiểm tra lại thư mục đích trước khi đặt bản clone vào vị trí đã chọn. Zest dùng luồng chuyển workspace hiện có để nạp dữ liệu, khởi động file watcher và thêm đường dẫn cục bộ vào **Recent Workspaces**. Giao diện Git hiển thị nhánh đã checkout và remote `origin`. Những lần mở lại từ **Recent Workspaces** dùng bản clone cục bộ và hoạt động cả khi offline.

### 4.2 Xử lý lỗi và trạng thái

- Clone và kiểm tra metadata chạy qua tầng Git và lưu trữ trên background executor. Mỗi hộp thoại chỉ có một tác vụ đang chạy. Người dùng có thể hủy trước khi Zest bắt đầu đặt bản clone vào vị trí cuối. Luồng chuyển workspace bắt đầu sau khi đặt bản clone thành công.
- Bản checkout cần để `.local/` cho dữ liệu trên máy hiện tại và dùng đường dẫn trực tiếp cho metadata, cấu hình Git ignore, collections và environments. Zest báo lỗi nếu các vị trí này chứa dữ liệu local đã được commit hoặc liên kết tệp.
- Các lỗi thiếu Git, xác thực thất bại, mất mạng, timeout hoặc đích không ghi được hiển thị trong hộp thoại với URL và đường dẫn để người dùng chỉnh rồi thử lại.
- Hủy hoặc thất bại giữ nguyên workspace đang mở và danh sách **Recent Workspaces**. Sau khi tiến trình Git đã dừng, Zest chỉ dọn dữ liệu tạm do chính lần mở đó tạo. Dữ liệu có sẵn ở đích được bảo toàn.

### 4.3 Tiêu chí chấp nhận

- Repo hợp lệ qua HTTPS hoặc SSH được mở với đúng metadata, collections và environments của nhánh mặc định. Có thể mở lại từ **Recent Workspaces** khi offline.
- Workspace có `workspace.yaml` hợp lệ và chưa có request mở được với cây collections trống.
- Repo thiếu hoặc có `workspace.yaml` sai schema báo đúng lỗi và giữ nguyên workspace đang mở.
- Có thể clone vào một thư mục cha đang chứa dữ liệu. Nếu thư mục con cùng tên repo đã có dữ liệu, xác thực thất bại hoặc người dùng hủy giữa lúc clone, Zest giữ nguyên dữ liệu có sẵn và cho phép thử lại.
