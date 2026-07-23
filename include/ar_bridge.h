/**
 * @file ar_bridge.h
 * @brief Jembatan C++ untuk interop memori dan bindings API dengan modul Rust Fizgravity AR.
 * Mendukung tracking VIO EKF, Face Mesh, Hand tracking, Physics, 3DGS, dan kolaborasi P2P.
 */

#ifndef AR_BRIDGE_H
#define AR_BRIDGE_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stdbool.h>

/**
 * @brief Jumlah konstanta mesh wajah (468 vertices standar MediaPipe/ARKit).
 */
#define FACE_MESH_VERTICES_COUNT 468

/**
 * @brief Jumlah parameter blendshapes wajah standar (52 ARKit blendshapes).
 */
#define FACE_BLENDSHAPES_COUNT 52

/**
 * @brief Jumlah sendi tangan standar (21 joints).
 */
#define HAND_JOINTS_COUNT 21

/**
 * @brief Titik 3D vertikal dalam ruang koordinat wajah lokal.
 */
typedef struct {
    float x;
    float y;
    float z;
} ArVertex3D;

/**
 * @brief Titik 2D koordinat tekstur.
 */
typedef struct {
    float u;
    float v;
} ArTexCoord2D;

/**
 * @brief Vertex wajah ter-interleave kontigu (Posisi 3D + Normal 3D + UV 2D).
 */
typedef struct {
    ArVertex3D position;
    ArVertex3D normal;
    ArTexCoord2D uv;
} ArFaceVertexInterleaved;

/**
 * @brief Jaring wajah 3D dan koefisien blendshape untuk FFI.
 */
typedef struct {
    ArFaceVertexInterleaved vertices[FACE_MESH_VERTICES_COUNT];
    float blendshapes[FACE_BLENDSHAPES_COUNT];
} ArFaceMesh;

/**
 * @brief Jaring leher 3D virtual hasil ekstrapolasi.
 */
typedef struct {
    ArFaceVertexInterleaved vertices[34];
    uint32_t indices[96];
} ArNeckMesh;

/**
 * @brief Sendi koordinat tangan 3D untuk FFI.
 */
typedef struct {
    ArVertex3D joints[HAND_JOINTS_COUNT];
    float confidence;
    int32_t is_right_hand;
} ArHandJoints;

/**
 * @brief Representasi 3D pose kamera melintasi FFI C-ABI.
 */
typedef struct {
    float position[3]; // X, Y, Z (dalam meter)
    float rotation[4]; // Kuaternion (W, X, Y, Z)
} ArPose;

/**
 * @brief Koefisien Harmonik Sferis (SH) Orde 2 untuk RGB (total 27 floats).
 */
typedef struct {
    float coefficients_r[9];
    float coefficients_g[9];
    float coefficients_b[9];
} ArSphericalHarmonics;

/**
 * @brief Data struktur representasi satu 3D Gaussian Splat.
 */
typedef struct {
    ArVertex3D position;
    ArVertex3D scale;
    float rotation[4];
    float opacity;
    float color_sh[9];
} ArGaussianSplat;

/**
 * @brief Kunci Voxel Hash spasial 3D untuk sinkronisasi desentralisasi P2P.
 */
typedef struct {
    int32_t x;
    int32_t y;
    int32_t z;
    float confidence;
} ArVoxelHashKey;

/**
 * @brief Menginisialisasi instansi baru dari Fizgravity AR Engine di sisi Rust.
 * @return Pointer mentah void* ke objek FizgravityEngine internal.
 */
void* fizgravity_engine_init(const char* model_path);

/**
 * @brief Memperbarui pelacakan frame AR.
 */
int32_t fizgravity_engine_update_frame(
    void* engine_ptr,
    float timestamp,
    const void* camera_data,
    int32_t width,
    int32_t height,
    const void* imu_data,
    const int32_t* face_box_ptr,
    ArPose* out_pose,
    ArSphericalHarmonics* out_lighting
);

/**
 * @brief Mengekstrak jaring wajah 3D terupdate.
 */
int32_t fizgravity_engine_get_face_mesh(void* engine_ptr, ArFaceMesh* out_mesh);

/**
 * @brief Mengekstrak jaring leher virtual hasil ekstrapolasi.
 */
int32_t fizgravity_engine_get_neck_mesh(void* engine_ptr, ArNeckMesh* out_neck);

/**
 * @brief Mengekstrak sendi tangan 3D terupdate.
 */
int32_t fizgravity_engine_get_hand_joints(void* engine_ptr, int32_t is_right, ArHandJoints* out_joints);

/**
 * @brief Menambahkan plane collider ke simulator fisika.
 */
int32_t fizgravity_engine_add_physics_plane(void* engine_ptr, int32_t id, float x, float y, float z);

/**
 * @brief Menjalankan langkah simulasi fisika.
 */
int32_t fizgravity_engine_step_physics(void* engine_ptr, float delta_time);

/**
 * @brief Mengekstrak data 3D Gaussian Splats untuk render pipeline.
 */
int32_t fizgravity_engine_get_gaussian_splats(void* engine_ptr, ArGaussianSplat* out_splats, int32_t max_count);

/**
 * @brief Mengepaskan (fitting) awan titik mentah ke dalam representasi 3D Gaussians.
 */
int32_t fizgravity_engine_fit_point_cloud_to_gaussians(void* engine_ptr, const ArVertex3D* points, int32_t count);

/**
 * @brief Memulai pemindaian perangkat P2P terdekat.
 */
int32_t fizgravity_engine_p2p_start_discovery(void* engine_ptr);

/**
 * @brief Sinkronisasi voxel delta keys dengan perangkat terdekat.
 */
int32_t fizgravity_engine_p2p_sync_voxels(void* engine_ptr, const ArVoxelHashKey* keys, int32_t count);

/**
 * @brief Mengimpor koordinat wajah nyata hasil deteksi Google ML Kit ke dalam shared state Rust.
 */
int32_t fizgravity_engine_set_face_mesh(void* engine_ptr, const ArVertex3D* vertices, const float* blendshapes);

/**
 * @brief Mengambil indeks segitiga triangulasi untuk bibir atas (Upper Lip).
 */
int32_t fizgravity_engine_get_upper_lip_indices(uint32_t* out_indices, int32_t max_count);

/**
 * @brief Mengambil indeks segitiga triangulasi untuk bibir bawah (Lower Lip).
 */
int32_t fizgravity_engine_get_lower_lip_indices(uint32_t* out_indices, int32_t max_count);

/**
 * @brief Mengambil estimasi suhu warna (Kelvin) dan intensitas cahaya sekitar.
 */
int32_t fizgravity_engine_get_ambient_cct_and_intensity(void* engine_ptr, float* out_temp, float* out_intensity);

/**
 * @brief Menghitung pergeseran koordinat specular gliter secara dinamis.
 */
int32_t fizgravity_engine_calculate_glitter_shimmer_shift(
    void* engine_ptr,
    float gyro_x,
    float gyro_y,
    float gyro_z,
    float dt,
    int32_t screen_rotation_degrees,
    float* out_shift_x,
    float* out_shift_y
);

/**
 * @brief Menghitung pemulusan batas hairline pada dahi secara dinamis.
 */
int32_t fizgravity_engine_calculate_hairline_blending(void* engine_ptr, float* out_alphas, int32_t max_count);

/**
 * @brief Menganalisis kondisi tekstur kulit, kerutan dahi, dan noda jerawat.
 */
int32_t fizgravity_engine_analyze_skin_health(
    const unsigned char* image_rgb,
    int32_t width,
    int32_t height,
    float* out_roughness,
    float* out_wrinkles
);

/**
 * @brief Melakukan kalibrasi intrinsik kamera online secara dinamis.
 */
int32_t fizgravity_engine_update_auto_calibration(
    void* engine_ptr,
    float image_w,
    float image_h,
    float depth_z,
    float* out_focal_length
);

/**
 * @brief Menghitung koefisien ambient occlusion (AO) dinamis untuk setiap vertex jaring wajah.
 */
int32_t fizgravity_engine_calculate_dynamic_ao(void* engine_ptr, float* out_ao, int32_t max_count);

/**
 * @brief Membebaskan memori objek FizgravityEngine.
 * @param engine_ptr Pointer ke FizgravityEngine yang akan dibebaskan.
 */
void fizgravity_engine_release(void* engine_ptr);

#ifdef __cplusplus
}
#endif

// Abstraksi berorientasi objek C++ tingkat tinggi untuk kemudahan integrasi
#ifdef __cplusplus
namespace fizgravity {

class Engine {
public:
    Engine(const char* model_path = nullptr) {
        m_engine_ptr = fizgravity_engine_init(model_path);
    }

    ~Engine() {
        if (m_engine_ptr != nullptr) {
            fizgravity_engine_release(m_engine_ptr);
        }
    }

    bool update(float timestamp, const void* camera_data, int32_t width, int32_t height,
                const void* imu_data, const int32_t* face_box_ptr,
                ArPose& out_pose, ArSphericalHarmonics& out_lighting) {
        if (m_engine_ptr == nullptr) return false;
        
        int32_t result = fizgravity_engine_update_frame(
            m_engine_ptr, 
            timestamp, 
            camera_data, 
            width,
            height,
            imu_data, 
            face_box_ptr,
            &out_pose, 
            &out_lighting
        );
        
        return result == 0;
    }

    bool getFaceMesh(ArFaceMesh& out_mesh) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_get_face_mesh(m_engine_ptr, &out_mesh) == 0;
    }

    bool getNeckMesh(ArNeckMesh& out_neck) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_get_neck_mesh(m_engine_ptr, &out_neck) == 0;
    }

    bool getHandJoints(bool is_right, ArHandJoints& out_joints) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_get_hand_joints(m_engine_ptr, is_right ? 1 : 0, &out_joints) == 0;
    }

    bool addPhysicsPlane(int32_t id, float x, float y, float z) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_add_physics_plane(m_engine_ptr, id, x, y, z) == 0;
    }

    bool stepPhysics(float delta_time) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_step_physics(m_engine_ptr, delta_time) == 0;
    }

    int32_t getGaussianSplats(ArGaussianSplat* out_splats, int32_t max_count) {
        if (m_engine_ptr == nullptr) return 0;
        return fizgravity_engine_get_gaussian_splats(m_engine_ptr, out_splats, max_count);
    }

    bool fitPointCloudToGaussians(const ArVertex3D* points, int32_t count) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_fit_point_cloud_to_gaussians(m_engine_ptr, points, count) == 0;
    }

    bool p2pStartDiscovery() {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_p2p_start_discovery(m_engine_ptr) == 0;
    }

    bool p2pSyncVoxels(const ArVoxelHashKey* keys, int32_t count) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_p2p_sync_voxels(m_engine_ptr, keys, count) == 0;
    }

    bool setFaceMesh(const ArVertex3D* vertices, const float* blendshapes) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_set_face_mesh(m_engine_ptr, vertices, blendshapes) == 0;
    }

    int32_t getUpperLipIndices(uint32_t* out_indices, int32_t max_count) {
        return fizgravity_engine_get_upper_lip_indices(out_indices, max_count);
    }

    int32_t getLowerLipIndices(uint32_t* out_indices, int32_t max_count) {
        return fizgravity_engine_get_lower_lip_indices(out_indices, max_count);
    }

    bool getAmbientCctAndIntensity(float& out_temp, float& out_intensity) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_get_ambient_cct_and_intensity(m_engine_ptr, &out_temp, &out_intensity) == 0;
    }

    bool calculateGlitterShimmerShift(float gyro_x, float gyro_y, float gyro_z, float dt,
                                      int32_t screen_rotation_degrees, float& out_shift_x, float& out_shift_y) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_calculate_glitter_shimmer_shift(
            m_engine_ptr, gyro_x, gyro_y, gyro_z, dt, screen_rotation_degrees, &out_shift_x, &out_shift_y
        ) == 0;
    }

    bool calculateHairlineBlending(float* out_alphas, int32_t max_count) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_calculate_hairline_blending(m_engine_ptr, out_alphas, max_count) == 0;
    }

    static int32_t analyzeSkinHealth(const unsigned char* image_rgb, int32_t width, int32_t height,
                                     float& out_roughness, float& out_wrinkles) {
        return fizgravity_engine_analyze_skin_health(image_rgb, width, height, &out_roughness, &out_wrinkles);
    }

    bool updateAutoCalibration(float image_w, float image_h, float depth_z, float& out_focal_length) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_update_auto_calibration(m_engine_ptr, image_w, image_h, depth_z, &out_focal_length) == 0;
    }

    bool calculateDynamicAo(float* out_ao, int32_t max_count) {
        if (m_engine_ptr == nullptr) return false;
        return fizgravity_engine_calculate_dynamic_ao(m_engine_ptr, out_ao, max_count) == 0;
    }

    // Hindari penyalinan objek mesin secara tidak sengaja untuk mencegah double free
    Engine(const Engine&) = delete;
    Engine& operator=(const Engine&) = delete;

private:
    void* m_engine_ptr = nullptr;
};

} // namespace fizgravity
#endif

#endif // AR_BRIDGE_H
